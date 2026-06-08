# 重构计划

## 目标

每个文件、每个函数只做一件事。每个 commit 只做一件事。

## 现状

| 文件 | 行数 | 方法数 | 问题 |
|------|------|--------|------|
| session_ops.rs | 2022 | 33 | tree 操作 + session 管理 + workspace 管理 + config 持久化 + 统计 + 预览 |
| ui_popup.rs | 2393 | 28 | 纯渲染，但每个 popup 10-100 行，单文件过大 |
| mod.rs | 2286 | 2 (pub) | App struct 35+ 字段 + 事件循环 + 测试 |
| extraction.rs | 1704 | ? | JSONL 解析 + diff 计算 + 标题提取 + token 提取 |
| ui.rs | 1611 | 7 | 渲染逻辑，相对内聚 |
| handler.rs | 1343 | 10 | 每个 handler 是大 match，直接操作 4 个域 |
| discovery.rs | 874 | 15 | 文件发现 + JSONL 解析 + session 构建 + 路径匹配 |
| handler_select.rs | 865 | ? | 选择器输入处理，相对内聚 |

## Phase 1：低风险拆分（纯提取，不改签名）

每个 commit 只拆一个文件。

### 1.1 session_ops.rs → 4 个模块

**方法分组：**

| 新文件 | 行数(估) | 方法 |
|--------|----------|------|
| `tree.rs` | ~300 | `rebuild_tree`, `move_sel`, `toggle_expand`, `navigate_to_session`, `toggle_selection` |
| `preview.rs` | ~300 | `start_session_preview`, `load_preview_summary`, `reload_preview_content`, `load_knowledge_preview`, `clear_workspace_knowledge` |
| `delete_ops.rs` | ~200 | `request_delete`, `confirm_delete`, `cancel_delete` |
| `session_ops.rs` (保留) | ~1200 | refresh, match_pty, stats, conflicts, budget, activate, poll_states, save_config, branch, diff, export 等 |

风险：低。全部是 `impl App` 方法移动，不改签名。只需要改 `mod` 声明。

### 1.2 ui_popup.rs → 3 个模块

| 新文件 | 内容 |
|--------|------|
| `ui_popup_select.rs` | agent/browser/theme/template/automation/chain/branch 选择弹窗 |
| `ui_popup_view.rs` | help/settings/keybind/stats/token_stats/diff/remote/timeline/conflict/recommend/cross_search/semantic_search 预览弹窗 |
| `ui_popup_confirm.rs` | delete/rollback/preflight/budget_warning 确认弹窗 |

风险：低。纯渲染函数移动，无逻辑依赖。

### 1.3 extraction.rs → 2 个模块

| 新文件 | 内容 |
|--------|------|
| `extraction.rs` (保留) | JSONL 解析：`parse_gsd_session`, `parse_codex_session`, `extract_*` |
| `session_title.rs` | 标题提取：`extract_claude_title`, `clean_user_message`, `extract_last_user_message` |

风险：低。纯函数移动。

## Phase 2：中风险重构（部分签名变更）

### 2.1 handler.rs 拆分大方法

当前 `handle_key` 是 230 行的 match。拆成：

- `handle_none_mode_key` — InputMode::None 下的按键
- `handle_input_mode_key` — 各种 InputMode 输入
- `handle_scrollback_key` — scrollback 搜索

每个方法控制在 50 行以内。

### 2.2 App struct 字段重组

把 35+ 字段组织成更细粒度的子结构体：

```rust
struct App {
    view: AppView,         // 已有
    ptys: PtyManager,      // 已有
    sessions: SessionStore,// 已有
    popup: PopupState,     // 已有
    chains: ChainState,    // 已有
    // 新分组：
    browse: BrowseState,   // browse_dir + browse_entries + browse_state
    search: SearchState,   // search_index + search_results + search_result_state
    io: IoState,           // last_refresh + timeline_events + agent_recommendations
}
```

减少 App 的扁平字段数。`BrowseState` 和 `SearchState` 已经有独立的 handler 文件。

### 2.3 discovery.rs 职责分离

| 新文件 | 内容 |
|--------|------|
| `discovery.rs` (保留) | `discover_sessions`, `discover_sessions_by_ids`, `discover_sessions_cached` — 公共 API |
| `discovery_walk.rs` | `collect_*`, `walk_*`, `find_session_jsonl*` — 文件遍历 |
| `discovery_parse.rs` | `parse_session_from_path`, `parse_gsd_session` (从 extraction 移来) |

## Phase 3：高风险重构（核心循环变更）

### 3.1 handler 方法接收具体参数

把 `fn handle_key(&mut self, key)` 改为接收需要的具体状态，而非整个 `&mut self`。

示例：
```rust
// Before
fn handle_sidebar_key(&mut self, key: KeyEvent) -> Result<Action>

// After — 不改，留在 Phase 3 因为需要全面回归测试
```

**这个阶段需要：**
- 全面的集成测试覆盖
- 手动回归测试清单
- 每个 refactor 后跑全量测试

## 执行原则

1. **一个 commit 只做一件事** — 一个文件拆分 = 一个 commit
2. **先测试后重构** — 每个 phase 开始前确认 350+ tests 全绿
3. **可随时停下** — 每个 commit 后代码都可编译、可运行、可发布
4. **不改行为** — 纯结构重组，不修 bug、不加功能

## 执行顺序

```
Phase 1.1  session_ops → tree.rs + preview.rs + delete_ops.rs
Phase 1.2  ui_popup → ui_popup_select + ui_popup_view + ui_popup_confirm
Phase 1.3  extraction → session_title.rs
Phase 2.1  handler.rs 大方法拆分
Phase 2.2  App 字段重组 (browse/search/io)
Phase 2.3  discovery → discovery_walk + discovery_parse
Phase 3.1  handler 方法签名优化
```

每一步：改代码 → cargo build → cargo test → commit。
