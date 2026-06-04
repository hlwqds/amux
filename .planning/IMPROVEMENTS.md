# amux 功能改进待办

> 基于 src/ 全量代码审查,仅收录有硬证据的空白或有硬伤的点。
>
> 生成日期: 2026-06-03

---

## 高优先级

### P1: Keybinds 配置已定义但未消费

`Keybinds` 结构体 (`types.rs:472-553`) 和 `Config.keybinds` 字段已完整,但 `handler.rs` 里所有按键都是硬编码匹配,从未读取 `self.keybinds`。

- [x] 在 `handle_key` 入口处,将所有 `KeyCode` 硬编码匹配替换为查 `self.keybinds` 表
- [x] 添加键位冲突检测(启动时 warn)
- [x] 在 Settings 弹窗中展示当前键位绑定 (k=keybinds)
- [x] 验收:用户可通过 `config.json` 的 `keybinds` 字段自定义任意键位,默认行为与当前硬编码一致

### P2: PTY Scrollback 键位未暴露

`PtyHandle` 已实现 `scrollback_offset()` / `scroll_page_up()` / `scroll_page_down()` / `reset_scroll()` (`pty.rs:147-169`),但无键位绑定。`IDLE_THRESHOLD_SECS` 和 `last_output_at` 已定义但未消费。

- [x] PTY 获焦时绑定 `PgUp`/`PgDn` 翻页 (已有)
- [x] `Home`/`End` 跳顶/底,`Ctrl+B`/`Ctrl+F` vi 风格翻页
- [x] 状态栏显示 idle 时长 (`now - last_output_at > IDLE_THRESHOLD_SECS`)
- [ ] 搜索功能重设计：1) `Ctrl+F` 触发 PTY scrollback 页内搜索（浏览器直觉），顶部覆盖搜索栏 + 匹配高亮 + n/N 跳转；2) `Ctrl+Shift+F` 触发全局搜索（跨 session 内容搜索，需重新设计交互：搜索框 → 结果列表 → 选中跳转到对应 session）；两者共享搜索栏 UI 组件，只在 Chat focus 且有 PTY 时可用
- [x] 可选:agent 空闲超阈值时触发 `notify-send` 通知 (已有桌面通知)

### P3: 打通 `amux serve` ↔ 正在运行的 PTY

TUI (`app::run()`) 和 serve 是两个独立进程,PTY 句柄无法跨进程共享。`server/api.rs` 只读磁盘历史,`ws.rs` 需要 `state.ptys` 里有句柄但 TUI 不注册。Web 客户端缺 input/resize API。

- [x] 将 axum server 作为 `app::run()` 内部 tokio task 启动,共享 `Arc<AppState>`
- [x] PTY 创建时自动注册到 `state.ptys`
- [x] 补 `POST /api/pty/{id}/input` 端点
- [x] 补 `POST /api/pty/{id}/resize` 端点
- [x] 补 `GET /api/ptys` 端点(列出正在运行的 PTY)
- [x] WS 改为发送 ANSI 原始字节流(take_raw_output drain + Message::Binary)
- [x] 验收:浏览器打开 `http://localhost:8080` 可看到正在运行的 PTY,可输入/resize/查看实时输出

---

## 中优先级

### P5: 插件系统输出管道增强

`Plugin` (`types.rs:228-235`) 和 `InputMode::PluginList`/`PluginOutput` 已接好,但插件只能展示纯文本。

- [x] 插件输出支持 ANSI 渲染(ansi_to_spans 解析器)
- [x] 插件返回结构化 JSON 时可触发 TUI action (`create_session` / `switch_workspace` / `notify`)
- [x] 插件注册为 session 生命周期 hook (`on_complete`)
- [x] 插件输出支持分页(j/k/PageUp/PageDown/Home/End 滚动)

### P6: RemoteHost SSH 远程 Session 发现

`RemoteHost` (`types.rs:213-224`)、`InputMode::RemoteView` 和渲染已就绪,但 `discovery.rs` 中无 SSH 连接逻辑。

- [x] 使用系统 `ssh` 命令连接远端主机扫描 session 目录
- [x] 远端执行扫描 `~/.claude/projects` 等目录(支持 agent_paths 自定义)
- [x] 结果标记 `[remote]` 以区分本地 session
- [x] SSH 连接失败时明确错误提示
- [x] 不需要(已有 Web 远程访问)

### P8: Post-completion Check 支持 Cargo 以外的构建系统

`mod.rs:379-411` 的 `CheckStatus` 只跑 `cargo test` + `cargo clippy`,硬编码为 Rust 项目。对 Node/Python/Go 项目会误报。

- [x] 检测 workspace 根目录的项目类型 (`package.json` → npm/pnpm, `pyproject.toml` → python, `go.mod` → go, `Makefile` → make)
- [x] 根据项目类型选择对应 check 命令 (`npm test`, `pytest`, `go test ./...`, `make test`)
- [x] 可在 `config.json` 中覆盖默认 check 命令 (`Config.check_command`)
- [x] 验收:Node 项目自动跑 `npm test`,Rust 项目跑 `cargo test + clippy`,Python 项目跑 `pytest`

### P9: Recording 回放

`mod.rs:262-272` 已写 asciicast v2 `.cast` 文件,`InputMode::RecordingList` 能列出文件名,但只能看文件列表,**不能播放**。

- [x] 选中 `.cast` 文件后 Enter 进入回放模式:按时间戳逐帧渲染到 PTY 视图
- [x] 支持暂停/继续(`Space`)、倍速(`/` / `*`)、跳转(`g`/`G`)
- [x] 不需要
- [x] 验收:选中 Recording 后可观看完整 session 回放,支持暂停和倍速

### P10: 自定义主题

`ThemeName` 枚举只有 `Dark`/`Light` 两个值 (`theme.rs:5-9`),`Theme` 结构体有 16 个颜色槽 (`theme.rs:37-72`),但颜色全部硬编码在 `Theme::dark()`/`Theme::light()` 里。

- [x] 支持从 `~/.local/share/amux/themes/{name}.json` 加载自定义主题
- [x] `ThemeName` 增加 `Custom(String)` 变体
- [x] Settings 弹窗中可浏览和切换已安装主题 (t=themes)
- [x] 验收:用户创建一个 JSON 主题文件后在 TUI 中可切换使用

### P11: Session 标签/分组

`Session` 有 `tags: Vec<String>` 字段 (`types.rs`) 和 `TagFilter` InputMode,但 handler 里只做了 `tag_filter` 的 UI,**无法给 session 打标签**。

- [x] Session Shift+I 进入 tag 编辑模式:输入逗号分隔的标签
- [x] 标签持久化到 `~/.local/share/amux/sessions/{id}.meta.json`(save_session_meta)
- [x] 侧边栏标签显示 + 按标签筛选(t key 已有)
- [x] 验收:用户可给任意 session 打标签,按标签筛选

---

## 低优先级

### P7: macOS 支持

依赖已是跨平台的 (`portable-pty`, `notify` + `macos_kqueue` feature),未发现 Linux-only 系统调用。README 声明 "Linux x86_64"。

- [x] macOS 上编译测试,修复路径差异 (data_dir 使用 ~/Library/Application Support/amux/)
- [x] 补 GitHub Actions matrix 构建 (linux + macos, x86_64 + aarch64) — README 已更新
- [x] 更新 README 移除 "Linux x86_64" 限制

### P12: Workspace 级 .amux.json 项目配置

当前所有配置全局在 `~/.local/share/amux/config.json`,无法按项目定制。团队协作时无法共享 agent 偏好、默认模板等。

- [x] 在 workspace 根目录识别 `.amux.json` 文件
- [x] 支持项目级配置:默认 agent、默认 prompt 模板、check 命令覆盖、忽略的 session
- [x] 加载优先级:项目配置 > 全局 config.json
- [x] `.amux.json` 可 git 跟踪,团队共享
- [x] 验收:在项目目录放 `.amux.json` 后,该 workspace 自动应用项目配置

### P13: Session 摘要自动展示

`mod.rs:369-378` 已在 session 完成时调用 `generate_session_summary` 写入 `~/.local/share/amux/summaries/{id}.md`,但这个摘要文件**从未在 TUI 中展示**。

- [x] Session 完成后自动弹出摘要预览(SummaryPreview 弹窗)
- [x] SessionPreview 弹窗 s 键切换 "Summary" tab
- [x] 摘要内容支持 markdown 渲染(标题/列表/代码块)
- [x] 验收:session 完成后自动看到摘要,也可在 preview 模式查看历史摘要

### P14: 文件冲突自动 Git Worktree 隔离

`mod.rs:552-611` 的 `detect_file_conflicts` 已能检测同 workspace 多 PTY 修改相同文件的冲突,但只能弹警告。

- [x] 检测到冲突时提供一键解决方案:自动为每个 PTY 创建 git worktree (I=Isolate / D=Dismiss)
- [x] PTY 在独立 worktree 中运行,避免文件冲突
- [x] session 完成后自动合并 worktree 回主分支(可选)
- [x] 验收:多 agent 同时工作时不再产生文件冲突


---

## 新增功能建议(第二轮)

> 项目完成度极高,P1-P14 几乎全部已实现。以下是基于 9000+ 行完整代码审查后发现的**真正空白方向**。

### P15: Session 变更快照与一键回滚

Session 完成时已记录 `GitInfo`(branch/commit/diff_stat)和 `DiffSummary`(files_changed/insertions/deletions),但用户无法从 TUI 内**回滚**这些变更。agent 跑坏时用户只能手动 `git checkout`。

- [x] Session 完成时自动创建 `amux/snapshots/{session_id}` 保存 session 开始前的 git HEAD commit
- [x] SessionPreview 弹窗增加 "Rollback" 操作:一键 `git reset --hard` 到 session 开始前的 commit
- [x] 侧边栏 completed session 显示 diff stat 摘要(+N/-M files changed)
- [x] Rollback 前弹出确认,显示将要丢弃的文件列表
- [x] 验收:agent 改坏了代码后,选中 session → Enter → Rollback → 代码回到 session 前的状态

### P16: Token/Cost 预算告警

`TokenUsage` 已提取 token 数和 cost(`discovery.rs:932-933`),`render_token_stats` 展示了汇总,但**没有阈值告警**。用户跑多个 agent 时可能烧掉大量 token 却不知情。

- [x] `Config` 增加 `token_budget: Option<TokenBudget>` (daily/weekly 预算)
- [x] 每次 refresh cycle 检查累计 token 消耗是否超预算
- [x] 超预算时状态栏红色闪烁警告 + 桌面通知
- [x] 可选:超预算后自动暂停所有 PTY(需用户确认)
- [x] 验收:设置 `"token_budget": {"daily_tokens": 1000000}` 后,当日累计超 1M tokens 时收到告警

### P17: Session Prompt 模板变量

`SessionTemplate` 有 `prompt: String` 和 `agent: Agent` 字段,但 prompt 是纯文本。无法引用 workspace 上下文、git diff、文件列表等动态内容。

- [x] 支持 `{git_diff}` — 自动插入当前 workspace 的 `git diff --stat` 输出
- [x] 支持 `{git_branch}` — 当前分支名
- [x] 支持 `{files_changed}` — 最近修改的文件列表
- [x] 支持 `{workspace_path}` — workspace 路径
- [x] 支持 `{project_type}` — 自动检测的项目类型(rust/node/python/go)
- [x] 验收:模板 `"Review these changes:\n{git_diff}"` 自动插入实际 diff 内容

### P18: Session 间上下文传递(Chain)

compare 功能已可同时启动多个 agent,但**无法串联**:让 A 的输出自动变成 B 的输入。当前只能手动复制。

- [x] `Config` 增加 `chains: Vec<SessionChain>`,定义 `[{agent: claude, prompt: "..."}, {agent: codex, prompt: "Review:\n{prev_output}"}]`
- [x] session 完成后自动提取 `extract_session_output` 作为 `prev_output` 变量
- [x] chain 中的后续 session 使用延迟注入 prompt
- [x] TUI 中显示 chain 进度:Step 2/3, 上一步完成状态
- [x] 验收:定义 chain 后一键启动,Claude 跑完自动把输出喂给 Codex review

### P19: Session 输出中的文件路径可点击跳转

`extract_file_paths` (`util.rs:254-262`) 已从 PTY 输出提取文件路径,`ui.rs:492` 调用了它,但只用于**展示**,不能跳转。

- [x] 在 PTY scrollback 视图中高亮可点击的文件路径
- [x] `g` 在路径上时打开文件(用 `$EDITOR` 或 `$VISUAL`)
- [x] 路径带行号时(`src/main.rs:42`)自动定位到行
- [x] 不需要
- [x] 验收:agent 输出 `src/main.rs:42 error` 时,光标移到该行按 Enter 即可打开编辑器

### P20: Session 分屏并排视图

多 PTY tab 已实现(`Ctrl+J/K` 切换),但**无法同时看两个**。compare 功能同时启动了多个 agent,却只能切着看。

- [x] `Ctrl+W` 进入分屏模式:上下/左右分割 PTY 视图
- [x] `Tab` 在两个分屏间切换焦点
- [x] 分屏各自 resize 到对应区域
- [x] compare 模式自动进入分屏(每个 agent 一个 pane)
- [x] 验收:compare 两个 agent 时可同时看到两边输出

### P21: 命令行 `amux` 非交互模式(Headless)

`mod.rs:1676-1678` 已检测非终端模式,但只做了自动关闭。无法在 CI/脚本中用 amux 批量管理 session。

- [x] `amux run --agent claude --prompt "fix the bug" --workspace ./proj` — 启动 agent 并等待完成,输出 stdout
- [x] `amux list --json` — JSON 输出所有 session
- [x] `amux status <session-id>` — 输出单个 session 状态
- [x] 退出码:0=成功,1=失败,2=超时
- [x] 验收:`amux run --agent claude --prompt "fix typo" && echo "done"` 可在 CI 中使用

### P22: Session 输出文件路径变更追踪

`detect_file_conflicts` 只检测**正在跑的** session 之间的冲突。无法知道一个**已完成**的 session 改了哪些文件,以及这些文件后续是否被其他 session 再次修改。

- [x] SessionPreview 中显示该 session 修改的文件列表 + 当前文件状态(已被其他 session 覆盖 / 未改动)
- [x] 文件行级别的变更归属:哪个 session 最后修改了这行(用 `git blame` / `git log`)
- [x] 可选:生成 workspace 变更时间线(哪个 session 在什么时间改了什么文件)
- [x] 验收:选中一个 completed session,看到它改了 5 个文件,其中 2 个已被后续 session 覆盖

### P23: Session 评分与质量追踪

`AgentMetrics` 只统计 session 数量(`compute_agent_recommendations`),没有质量维度。用户无法知道哪个 agent 在自己的项目上表现更好。

- [x] Session 完成时用户可打分:1-5 星(快捷键 `*` × N 次)
- [x] 分数持久化到 `sessions/{id}.meta.json` 的 `rating` 字段
- [x] `AgentRecommend` 弹窗增加质量维度:按平均评分排序(而非只按数量)
- [x] 可选:自动评分启发式 — check_status=Passed 加 1 分,无 git diff 减 1 分
- [x] 验收:AgentRecommend 弹窗显示"平均评分 4.2 ★ (12 sessions)",而非纯数量

### P24: Session Note/评论

Session 有 `tags` 但没有自由文本笔记。用户无法记录"这个 session 改了什么、为什么失败"。

- [x] SessionPreview 增加 `n=note` 操作:进入自由文本编辑
- [x] 笔记持久化到 `sessions/{id}.meta.json` 的 `note` 字段
- [x] 侧边栏 session 列表中,有笔记的 session 显示 `📝` 标记
- [x] 笔记参与 `cross_session_search` 的全文搜索
- [x] 验收:选中 session → preview → n → 输入笔记 → 下次打开能看到

### P25: `amux doctor` 环境诊断

用户首次使用时经常遇到 agent 未安装、git 不可用、权限问题等。没有统一诊断命令。

- [x] `amux doctor` 子命令检查:git 是否可用、各 agent CLI 是否在 PATH、数据目录权限、session 目录是否存在
- [x] TUI 内 `?` Help 弹窗底部增加 "Run `amux doctor` for diagnostics" 提示
- [x] 首次启动时自动运行精简版 doctor,有问题在状态栏显示警告
- [x] 验收:`amux doctor` 输出各项检查结果(✓/✗),未安装 agent 时给出安装命令提示

### P26: Compare 智能分析(非纯 diff)

`DiffView` 是逐行文本 diff,compare 多个 agent 时无法快速判断"谁做得更好"。

- [x] Compare 完成后自动生成对比摘要:各 agent 改了几个文件、insertions/deletions 数、check_status 结果
- [x] 并排展示各 agent 的 diff stat(而非纯文本 diff)
- [x] check_status 结果差异高亮(A passed, B failed)
- [x] 验收:compare 完成后弹窗显示 "Claude: 3 files +120/-30 ✓ | Codex: 5 files +200/-80 ✗"

### P27: Session Replay Prompt(复用历史 prompt)

用户经常重复类似任务。当前只能手动重输或用静态 template,不能直接复用历史 session 的 prompt。

- [x] 选中历史 session → `r=replay` → 自动复制该 session 的第一个 user message 作为新 session prompt
- [x] 使用 `expand_template_vars` 展开变量(如 `{git_diff}` 变为当前 diff)
- [x] 不需要
- [x] 验收:选中之前的 "fix login bug" session → r → 新 session 以相同 prompt + 当前 git diff 启动

### P28: 桌面集成 — 系统托盘/状态图标

amux 长时间运行时没有系统级状态指示。用户不知道 agent 是否在跑、是否完成了。

- [x] 集成系统托盘(Waybar/polybar JSON 状态文件 + named pipe):图标显示当前 agent 数量
- [x] agent 完成时状态文件更新
- [x] 状态文件支持右键菜单(waybar on-click 打开终端)
- [x] 可选:Waybar/polybar 自定义模块输出(写 JSON 到 named pipe + tray-status.json)
- [x] 验收:amux 运行时系统状态文件实时更新,agent 完成时 waybar 显示变化

---

## 清理计划:删除花里胡哨的功能

> 以下功能经评估实用性低,计划从代码中移除。自定义主题保留。

### 删除: Compare 功能(整体移除)

Compare 是 demo 功能,实际工作流中几乎没人用:烧 2-4x token、diff 比不出质量、合并是伪需求、真实工作流是串行迭代而非并行当裁判。

- [x] 删除 `InputMode::CompareSelect`、`InputMode::ComparePrompt`、`CompareSummary`
- [x] 删除 handler.rs 中 compare 相关按键和处理逻辑
- [x] 删除 ui.rs 中 compare 相关渲染
- [x] 删除 types.rs 中 compare 相关类型/字段(`CompareAgentSummary`)

### 删除: P20 分屏视图

分屏主要是为 compare 服务的,compare 删了就没意义。tab 切换够用。

- [x] 删除 `SplitMode` 枚举、`split_mode`、`split_ptys`、`split_focus` 字段
- [x] 删除 `toggle_split_mode`、`switch_split_focus`、`render_split_panes` 等方法
- [x] 删除 handler.rs 中 `Ctrl+W` 分屏逻辑
- [x] 删除 ui.rs 中分屏渲染逻辑

### 删除: P9 Recording 回放

录了没人看。调试用 log,演示用截图。

- [x] 删除 `InputMode::RecordingList`、`InputMode::RecordingPlayback`
- [x] 保留 `.cast` 文件写入,删除 TUI 回放逻辑
- [x] 删除 playback 状态字段和 `advance_playback` 方法
- [x] 删除 `parse_asciicast()` 和 `AsciicastFrames` 类型

### 删除: P22 文件变更追踪

`git log`/`git blame` 已经能做,不需要在 TUI 里重复实现。

- [x] 删除 SessionPreview 中的文件追踪 tab (`f` key)
- [x] 删除 `FileChangeInfo` 结构体和相关 `git blame`/`git log` 调用逻辑

### 删除: P23 Session 评分
已删除: `*` 键打分、自动评分启发式、AgentRecommend 评分显示、`save_session_rating`

### 删除: P24 Session Note
已删除: `InputMode::NoteEdit`、`n` 键笔记编辑、`save_session_note`、📝标记、笔记搜索

### 删除: P26 Compare 智能分析
已删除: `CompareSummary` InputMode 和 `CompareAgentSummary` 结构体

### 删除: P28 系统托盘
已删除: `src/tray.rs` 模块、tray 状态文件逻辑

### 删除: P11 标签编辑

有了但低频,search/filter 更实用。保留标签显示,删除手动编辑功能。

- [x] 删除 `Shift+I` tag 编辑模式、`InputMode::TagEdit`、`tag_edit_target` 字段
- [x] 保留 tags 字段和 TagFilter 筛选(只读,从 session 数据自动提取)


---

## 下一步方向:减法 + 质量

> 功能已经覆盖了日常使用的完整链路(启动 → 监控 → 回看 → 检查 → 回滚 → 诊断)。
> 继续加功能是边际收益递减。方向应该是减法 + 质量。

### 执行清理计划

- [x] 删除 compare、分屏、recording 回放、文件追踪、评分、笔记、托盘、标签编辑

### 补 `amux doctor` 环境诊断

唯一还没做且有实际价值的新功能。降低新用户上手门槛。

- [x] `amux doctor` 子命令:检查 git 可用性、各 agent CLI 是否在 PATH、数据目录权限
- [x] TUI Help 弹窗底部增加 "Run `amux doctor` for diagnostics"
- [x] 首次启动自动运行精简版,有问题状态栏警告

### 稳定性:消灭 unwrap/expect

- [x] 替换 `browse.rs:35` 的 `parent().unwrap()` 为 `parent().unwrap_or()` 安全降级
- [x] 替换 `browse.rs:97` 的 `as_ref().unwrap()` 为 match 安全降级
- [x] 替换 `pty.rs:127` 的 `take_writer().unwrap()` 为 match 错误处理
- [x] 替换 `ui.rs:2371` 的 `chars.next().unwrap()` 为 `unwrap_or('\\0')` 安全检查
- [x] 替换 `watch.rs:34` 的 `.expect()` 为 Option+日志降级(不 panic)

### 错误提示优化

- [x] agent 启动失败时给出可操作提示(`Agent::install_hint()`)
- [x] workspace 目录不存在时显示 WorkspaceWarning 黄色警告节点
- [x] git 操作失败时区分 "not a git repo" / "detached HEAD" 给不同建议(`git_cmd()` helper)

### 文档

- [x] README 补充 `amux doctor` 用法和 headless CLI 用法
- [x] README 补充 headless CLI 用法 (`amux run` / `amux list` / `amux status`)
- [x] 补写 `docs/config.md`:config.json 完整字段说明 + 示例
- [x] 补写 `docs/chains.md`:Session Chain 配置和使用场景

---

## 性能优化:主循环卡顿

> 主循环每 50ms 一个 tick,每个 tick 串行执行:render → poll_states → detect_paths → refresh_sessions → poll(event)。
> 以下几个操作是明确瓶颈。

### 优化 1: Recording 写入 — 每帧读 screen + hash + 写文件

`poll_states()` (mod.rs:382-404) 对**每个运行中的 PTY**,每帧都:
1. 读 screen `guard.screen().contents()` (拷贝整个终端内容)
2. hash 内容
3. 变了就 `fs::OpenOptions + append + write` 写 `.cast` 文件

这是每帧对每个 PTY 做 1 次 screen clone + 1 次 hash + 可能 1 次磁盘 IO。多个 PTY 时更严重。

- [x] 降低 recording 帧率:每 200ms 才执行一次 screen clone + hash + 写入(从每帧 50ms 降为 200ms)
- [x] 或:recording 默认关闭,用 `config.record_sessions: bool` 开启(N/A,保留当前行为)
- [x] 或:保留 hash 变化检测 + 200ms 节流双保险

### 优化 2: `detect_paths_from_screen` — 每帧 regex 扫描

mod.rs:1877-1884 每帧对活跃 PTY 的 screen 内容跑 `extract_file_paths_with_lines`,内部用 regex 扫描全文。screen 内容可能很大(几十 KB)。

- [x] 路径检测功能已在清理阶段移除,无需优化
- [x] N/A: `detect_paths_from_screen` 和相关代码已删除

### 优化 3: `refresh_sessions` — 每 5 秒扫描所有 agent session 目录

refresh_sessions (mod.rs:759-798) 每次调用 `discover_sessions()`:
1. 遍历 4 种 agent 的 session 目录(Claude/Codex/GSD/OMP)
2. 对每个 `.jsonl` 文件 `fs::metadata` + 可能 `fs::read_to_string` 提取 title
3. 过滤 ignore_patterns
4. `detect_file_conflicts()` — 对同 workspace 的运行 PTY 调 `git diff --name-only`
5. `check_token_budget()` — 遍历所有 session 提取 token usage

session 数量多时(>100)这会非常慢,尤其是 Codex/GSD 的 walk 目录逻辑。

- [x] `detect_file_conflicts` 移到 30s 低频定时器,不再每次 refresh 执行
- [x] `check_token_budget` 移到 30s 低频定时器,不再每次 refresh 执行
- [x] title 缓存待实现(当前已足够快)
- [x] `detect_file_conflicts` 和 `check_token_budget` 已通过独立定时器解耦

### 优化 4: `render_chat` — 每帧 `screen().clone()`

ui.rs:496-498 每帧 clone 整个 PTY screen 结构体用于渲染。大终端(200+ 列,几千行 scrollback)时 clone 成本可观。

- [x] 移除 `screen().clone()` — 直接传 `guard.screen()` 引用给 PseudoTerminal,零拷贝渲染
- [x] tui-term 的 PseudoTerminal 接受引用,无需 clone

### 优化 5: `refresh_sessions` 中的 `load_project_config` — 每 5 秒读文件

mod.rs:760-765 每次 refresh 都 `load_project_config` 读 `.amux.json`。这个文件几乎不变。

- [x] 缓存 `.amux.json` 的 mtime,只在文件变化时重新加载(`project_config_mtimes` HashMap)
- [x] 保留 cleanup 逻辑,移除已删 workspace 的缓存条目

---

## 重构计划:性能与稳定性

> 基于实际卡顿排查和代码审计。分三批执行:性能热路径 → IO 密集 → 健壮性。

### 第一批:主循环热路径(已修复 + 待修)

- [x] `poll_states` 的 `pre_session_map` — 每帧对全部 session 调 `find_session_jsonl`(Codex/GSD 嵌套 walk + read_to_string),改为仅 chain 活跃时构建
- [x] `detect_paths_from_screen` — 每帧 regex 扫描,改为 hash 跳过未变化帧
- [x] `render_chat` 的 `screen().clone()` — 每帧 clone 整个 screen,改为传引用
- [x] `load_project_config` 每 5 秒读文件 — 改为 mtime 缓存
- [x] `render_sidebar` 消除每帧 Vec 分配 — 预计算 pty_state_map 和 active_tab_data,避免闭包 borrow 冲突
- [x] `build_tab_bar` 预分配 with_capacity — `&mut self` 复用 buffer,预计算状态
- [x] 主循环 poll 间隔 — 自适应 100ms(空闲)/50ms(PTY 活跃),screen 未变时跳过 render

### 第二批:discover_sessions IO 优化

- [x] `extract_claude_title` — 改为 `BufReader` 逐行读,找到即停,避免读整个 jsonl
- [x] `walk_codex_jsonl` / `walk_gsd_jsonl` — 改为文件名精确匹配,不再 read_to_string + contains
- [x] `discover_sessions` — mtime 缓存增量 diff(`session_cache` HashMap),只重新解析变化的文件
- [x] `check_token_budget` — 已拆出为独立 30s 定时器
- [x] `detect_file_conflicts` — 已拆出为独立 30s 定时器

### 第三批:错误处理与健壮性

- [x] `browse.rs` 两处 unwrap — 已改为 unwrap_or 安全降级
- [x] `pty.rs:127` take_writer().unwrap() — 已改为 match + log 降级
- [x] `ui.rs` chars.next().unwrap() — 已改为 unwrap_or('\\0')
- [x] `watch.rs:34` .expect() — 已改为 Option + log 降级(不 panic)
- [x] session 完成 6 个 git 命令 — 合并为 4 个(diff --stat 复用 + diff --numstat 包含 name-only)
- [x] `detect_file_conflicts` / `check_token_budget` 不阻塞 UI — 已拆为 30s 独立定时器

---

## 结构性问题

> 代码功能完整但维护性已到瓶颈。核心问题是 App 上帝对象。

### 问题 1: App 上帝对象 — 130 字段平铺

`App` struct 有 ~130 个字段,全部平铺。每加一个功能要改 3 处:struct 定义 + `new()` 构造 + `test_app()` helper。已经到了改一处怕牵连其他的地步。

应该拆分为独立的状态子结构:

- [x] `AppView` — focus, input_mode, status, sort_mode, agent_filter, tag_filter, search_query, selected_set, last_chat_area, tab_bar_rect, theme, keybinds, screen_changed, prev_input_mode
- [x] `PtyManager` — ptys, active_pty, pty_counter, prev_states, pending_inputs, detected_paths, selected_path_idx, scroll_search_*
- [x] `SessionStore` — sessions, archived_sessions, show_archived, archive_days, session_cache, project_configs, project_config_mtimes, workspaces, tree, ws_session_map, tree_state
- [x] `PopupState` — preview_session_id, preview_lines, preview_show_summary, diff_lines, diff_left_session, branch_points, conflict_warnings, rollback_*, budget_alert, budget_flash_on
- [x] `ChainState` — chains, active_chain, chain_state

重构后 `App` 只持有这几个子结构 + 工作区列表 + 配置。每个子结构可以独立测试。

### 问题 2: 已删除功能的残留字段

清理计划中删除了文件追踪/Note/Tray/Compare/分屏/Recording,但字段可能还在:

- [x] 检查 `file_change_info` / `preview_show_files` — 已删除字段、方法、渲染代码
- [x] 检查 `note_edit_target` / `note_buffer` — 已删除字段、方法、渲染代码
- [x] 检查 `tray_completed` — 已删除字段及相关逻辑
- [x] 检查 `InputMode::NoteEdit` — 已删除变体及所有引用
- [x] 清理 handler.rs / ui.rs 中对应的 dead code — 已删除 handle_note_edit_key, render_note_edit, compute_file_changes, check_file_overwritten 等

### 问题 3: InputMode 36 个变体

`InputMode` 枚举有 36 个变体,每个弹窗一个。handler.rs 是巨大的 `match input_mode`,每加一个弹窗要改 handler + ui + types 三处。

- [x] 评估:合并为几大类 — **结论:保持现状(Option C)**。每个变体有独立的键处理/渲染/确认逻辑,合并只会将复杂度从 enum 转移到内部 match,不减少总代码量。flat enum 零开销、易搜索、易扩展
- [x] 评估记录在 src/types.rs InputMode 上方注释

### 问题 4: handler.rs 1800 行单文件

handler.rs 1797 行,包含所有按键处理逻辑。和 ui.rs(2553 行)加起来 4300 行 UI 代码,难以维护。

- [x] 按 InputMode 分组拆分 — handler.rs(793行) + handler_search.rs(234行) + handler_select.rs(713行)
- [x] 依赖问题 1 的 App 拆分 — 已完成(AppView/PtyManager/SessionStore/PopupState/ChainState)

### 问题 5: test_app() helper 130 行构造

`test_app()` 函数(mod.rs:2295-2389)有 130 行,逐字段构造 App。每加一个字段就要改这里。

- [x] 用 `Default` trait 替代手动构造 — test_app() 从 ~130 行简化为 ~15 行,使用 `..Default::default()` 只覆写测试相关字段
- [x] 所有子结构(AppView/PtyManager/SessionStore/PopupState/ChainState)均实现 Default

### 问题 6: WS 仍在 200ms 轮询

> 第二批重构里写了"WS 发送 ANSI 原始字节流"(`take_raw_output` 替代 `screen().clone()`),但 `ws.rs:43` 仍然用 `tokio::time::interval(200ms)` 轮询。改成事件驱动需要 PtyHandle 暴露 mpsc::Receiver 通知新数据到达。

- [x] `PtyHandle` 内部用 `Arc<tokio::sync::Notify>` 在 reader thread 新数据到达时通知
- [x] `ws.rs` 改为 `notify.notified()` + 5s heartbeat fallback,不再 200ms 轮询
- [x] 服务端 CPU 从 5Hz 永久唤醒降为仅在有数据时唤醒

### 问题 7: `PtyHandle::write_input` 静默丢弃错误

> `pty.rs:152` `let _ = self.writer_tx.send(...)`,PTY 死后(channel 关闭)客户端输入直接丢,UI 无任何反馈。Web 用户在浏览器敲字看不到任何错误。

- [x] write_input 改为返回 `Result<(), String>` — alive 检查 + try_send 错误
- [x] WS handler 收到 Err 后给客户端发 `[error: PTY closed]` 消息

### 问题 8: WS 初始 screen 用 Text,后续用 Binary

> `ws.rs:32-37` 初始 screen 内容用 `Message::Text` 发,后续每次 tick 用 `Message::Binary` 发 ANSI 字节。客户端(xterm.js)要处理两种不同类型,容易出错。

- [x] 初始 screen 改为 `Message::Binary(initial.into())`,客户端统一按 Binary 解析

### 问题 9: 认证 token 用 `==` 比较(timing attack)

> `auth.rs:22,31` `val == format!("Bearer {}", expected_token)` 和 `token == expected_token` 是字符串 `==`,Rust 的 `==` 是 O(n) 字节比较,理论上可通过计时攻击推断 token 长度。

- [x] 用 `subtle::ConstantTimeEq` 做常量时间比较 — Bearer header + query param 两处
- [x] 新增 `subtle = "2"` 依赖

### 问题 10: PtyHandle writer_tx 是 unbounded

> `pty.rs:30` `writer_tx: Sender<Bytes>` 默认 unbounded,PTY 死后客户端继续疯狂输入会让 channel 持续增长(直到 OOM)。虽然 `write_input` 丢弃了 send 错误,但 `Bytes::from(data.to_vec())` 的分配仍然发生。

- [x] 改为 `std::sync::mpsc::sync_channel(1024)` bounded channel + `try_send`
- [x] writer thread 用 `recv_timeout(5s)` 避免永久阻塞,write_input 返回 Err 满时

---

## Alt 快捷键重构

> 当前侧边栏快捷键用裸键(j/k/n/d/r/q/e/v/t/?/),PTY 获焦时容易和 agent 输入冲突。
> 统一改为 Alt+键,裸键全部转发给 PTY。不与 niri 冲突(niri 用 Super/Win 键)。
> 保留 PTY 内的 Ctrl 组合键(Ctrl+J/K 切 tab、Ctrl+B/F 翻页)不变。

### 步骤 1: KeyBinding 增加 alt 字段

当前 `KeyBinding` 只有 `ctrl`/`shift`,需要加 `alt`。

- [x] `types.rs` `KeyBinding` 增加 `#[serde(default)] pub alt: bool`
- [x] 增加 `KeyBinding::alt(key)` 构造函数
- [x] `matches_event()` 增加 ALT 修饰符检查
- [x] `display()` 增加 `Alt+` 前缀
- [x] 所有 17 个默认键位改为 `KeyBinding::alt(key)` (裸键 → Alt+键)

### 步骤 2: 更新默认键位映射

避免 Alt+字母冲突(同一个 Alt+key 只能绑一个动作):

| 功能 | 现在 | 改为 | 说明 |
|------|------|------|------|
| move_up | j | Alt+j | |
| move_down | k | Alt+k | |
| new_session | n | Alt+n | |
| delete | d | Alt+d | |
| refresh | r | Alt+r | |
| quit | q | Alt+q | |
| expand | e | Alt+e | |
| preview | v | Alt+v | |
| search | / | Alt+/ | |
| help | ? | Alt+h | 原 ? 需要 Shift 键,改为 Alt+h |
| settings | Shift+S | Alt+s | |
| theme | Shift+T | Alt+t | |
| rename | Shift+R | Alt+m | r 已用,用 m (modify) |
| export | Shift+E | Alt+x | e 已用,用 x (export) |
| copy | Ctrl+Y | Alt+y | |
| tag_filter | t | Alt+f | t 已用,用 f (filter) |
| new_workspace | Shift+N | Alt+w | |

### 步骤 3: handler.rs 侧边栏裸键全部移除

侧边栏获焦时,handler.rs:212-235 的 `kb.xxx.matches_event(&key)` 已经通过 keybinds 查表。
更新默认值后,裸键不再匹配(因为默认都带 Alt),自然全部进入 fallback。

- [x] 侧边栏裸键自然不再匹配 keybinds(默认都带 Alt),落入硬编码 fallback(template/replay/sort等非 keybinds 功能)
- [x] 侧边栏裸键处理非 keybinds 功能(template/replay/plugin/automation 等),不转发给 PTY

### 步骤 4: PTY 获焦时的裸键行为

PTY 获焦时,裸键应该全部转发给 agent(当前已经是这样)。需要确保 Alt+键在 PTY 模式下也被 amux 消费而非转发。

- [x] PTY 获焦时,Alt+键拦截 amux 操作(quit/tab/refresh/new/search/help/preview/export/copy/theme/settings/tag_filter)
- [x] PTY 获焦时,裸键和 Ctrl+键转发给 PTY(保持现状)
- [x] Ctrl+J/K(切 tab)、Ctrl+B/F(翻页)、Ctrl+Q(关闭 session)保持 Ctrl 不变

### 步骤 5: 冲突检测更新

`Keybinds::validate()` 已有冲突检测,但新增 alt 字段后需要更新比较逻辑。

- [x] `validate()` 增加 `kb_a.alt == kb_b.alt` 比较
- [x] `display_lines()` 已含 Alt+ 标识

### 步骤 6: 向后兼容

已有 config.json 可能包含旧格式(无 alt 字段)。

- [x] `#[serde(default)]` 在 alt 字段上确保旧配置文件 `alt: false` → 裸键模式仍然工作
- [x] 新安装默认 Alt+键,旧配置保持裸键兼容


---

## 新方向:解决用户真实痛点

> 不是给 TUI 加按钮,而是解决 agent 编码工作流中的实际痛点。

### P29: 会话知识库 — 跨 Session 上下文共享

**这是最大的 token 浪费点。** 每个 session 都从零开始理解代码库,80% 的 token 花在文件探索上。amux 已经有所有 session 的 JSONL,里面有大量代码理解结果。

场景: Claude 在 session #1 花了 50K tokens 理解了项目架构。用户开 session #2 让 Codex 修 bug。amux 自动把 session #1 的摘要注入到 Codex 的 prompt 里,token 消耗降低 50-80%。

- [x] 每个 workspace 维护 `knowledge.json` — src/knowledge.rs, 自动从完成 session 提取路径/技术/问题/架构
- [x] 新 session 启动时自动注入知识 — 通过 PendingInput 延迟注入(3s)
- [x] `knowledge.json` 结构 — `{ architecture, key_files, tech_stack, known_issues, last_updated }`
- [x] SessionPreview 'k' 查看/'c' 清除 knowledge 内容
- [x] 增量合并 — `merge_from_session()` 提取路径/技术/问题,去重
- [x] `.amux.json` 配置 `auto_inject_knowledge: false` 禁用(默认 true)
- [x] 验收通过 — 新 session 自动携带 workspace 知识

### P30: 预检(Pre-flight Check)

用户经常在错误的状态下启动 agent:未提交的更改、测试已经挂了、分支不对。agent 花了 5 分钟才发现环境有问题。

- [x] session 启动前自动检查 — git status/branch, .amux.json, cargo check
- [x] 检查结果以弹窗展示 — `InputMode::PreflightConfirm`, ✓/⚠/✗ 图标
- [x] Proceed/Fix first — Fix 执行 git stash 并重新检查
- [x] `.amux.json` 配置 `preflight.require_clean_git` / `preflight.mode`
- [x] 支持 "popup"/"silent" 两种模式
- [x] 验收通过 — dirty git state 下启动弹窗警告

### P31: 语义搜索 — 跨 Session 内容搜索

当前 `CrossSearch` 是纯文本 regex 搜索。用户记不清关键词,但记得意思:"那个 session 里我们讨论了认证流程的重构"。

- [x] 采用 TF-IDF + BM25 纯 Rust 方案(无外部依赖) — src/search_engine.rs
- [x] `InputMode::SemanticSearch` — 输入查询,j/k 选择结果,Enter 预览
- [x] SearchIndex 存储 — 可序列化,增量更新
- [x] 首次索引 — `rebuild_search_index()` 遍历所有 session summary
- [x] 增量更新 — refresh_sessions 时自动重建
- [x] BM25 评分 — k1=1.2, b=0.75, IDF 标准 formula
- [x] 渲染 — 查询输入框 + 结果列表(title + 分数百分比)
- [x] 纯 Rust 无外部依赖 — 14 个单元测试
- [x] 验收通过 — BM25 ranking 测试通过

---

## P32: 移动端 Web 改进

> 当前 Web 客户端有基础响应式和 xterm.js 终端,但手机上体验差:终端太小、没有功能键、无法创建 session。

### 功能改进

- [x] 虚拟功能键栏 — 终端底部 7 个按钮(ESC/TAB/↑/↓/Ctrl-C/Ctrl-D/Ctrl-Z),36px 固定高度
- [x] 新建 session UI — header "+" 按钮 → 模态弹窗(agent 选择 + name + workspace + prompt) → POST /api/sessions
- [x] 终端字体自适应 — <480px→10px, 480-768→12px, >768→14px, 自动 fit
- [x] 登录页 — 401 时显示 token 输入卡片,存 localStorage, Authorization header + WS query param
- [x] 键盘避让 — visualViewport.resize 监听,隐藏功能键栏,终端区自适应
- [x] PTY 状态指示 — 运行=绿●, 空闲=黄●, 完成=灰○
- [x] 横屏优化 — landscape+≤1024px 时 sidebar 折叠为 48px icon bar

---

## 移动端 Web 设计规范

> 供 open-design 生成 UI 参考的设计描述。

### 页面结构

三个页面:登录页、主页面(终端+侧边栏)、新建 Session 弹窗。

### 设计风格

- 暗色主题,与 TUI 的 Tokyo Night 配色一致
- 背景色 `#1a1b26`,卡片/面板 `#1f2335`,边框 `#3b4261`
- 主色调蓝 `#7aa2f7`,成功绿 `#9ece6a`,警告红 `#f7768e`
- 圆角 4-8px,间距紧凑(4-8px)
- 等宽字体用 JetBrains Mono / Fira Code,UI 字体用系统字体
- 无大图无装饰,纯功能性界面

### 页面 1: 登录页

- 全屏居中一个卡片
- 卡片内:amux logo(文字即可)、一个 token 输入框、"Connect" 按钮
- 底部小字:"Token is set in config.json → serve_token"
- 输入框获焦时不缩放(iOS 需 font-size ≥ 16px)
- 无 token 配置时此页跳过,直接进主页面

### 页面 2: 主页面(竖屏手机)

从上到下布局:

**顶部栏(固定)** — 高 44px
- 左侧:amux 文字 logo
- 中间:当前 session 标题(截断)
- 右侧:连接状态指示灯(绿点=connected, 红点=disconnected)

**Session 列表(可折叠)** — 高度 30vh,可下拉展开到 60vh
- 每行:agent 标签(小色块) + session 标题 + 状态点(running=绿, completed=灰, idle=黄)
- 顶部有 "+" 按钮新建 session
- 点击 session 连接 WebSocket 并渲染终端
- 左滑 session 可删除(仅 completed)

**终端区域(xterm.js)** — 剩余高度
- 黑色背景,等宽字体,自适应 fontSize
- 终端底部有虚拟功能键栏

**虚拟功能键栏(固定)** — 高 36px
- 一排按钮:ESC | TAB | ↑ | ↓ | Ctrl+C | Ctrl+D
- 按钮紧凑排列,暗色背景,按下时高亮
- 可左滑展开更多:Ctrl+Z | Ctrl+L | Ctrl+A

**输入栏(固定)** — 高 40px
- 输入框 + 发送按钮
- 输入框 font-size: 16px(防 iOS 缩放)
- 发送后自动 append \n

### 页面 2: 主页面(横屏手机)

- Sidebar 折叠为左侧 icon bar(48px 宽):每个 session 显示 agent 图标首字母
- 终端占满剩余宽度
- 虚拟功能键栏仍在底部
- 输入栏仍在底部

### 页面 3: 新建 Session 弹窗(模态)

- 全屏模态,暗色半透明遮罩
- 弹窗卡片,圆角 12px
- 内容:
  1. Agent 选择:横向排列 agent 按钮(Claude / Codex / GSD / OMP),选中高亮
  2. Prompt 输入:textarea,3-5 行高
  3. "Start" 按钮(主色调蓝)
  4. 右上角 "×" 关闭
- Start 后调用 `POST /api/sessions`,成功后自动连接新 session

### 交互细节

- 触摸滑动切换 session(左滑下一个,右滑上一个),已有实现保留
- 终端内容双指缩放(fontSize 在 8-16px 间调整)
- 下拉刷新 session 列表
- agent 标签颜色:Claude=橙色, Codex=蓝色, GSD=紫色, OMP=绿色
- 长按 session 弹出上下文菜单:Preview / Rollback / Delete

### 技术约束

- 单文件 HTML(inline CSS + JS),embed 到 Rust 二进制
- 依赖仅 xterm.js + xterm-addon-fit(从 CDN 加载)
- 不用构建工具,不用框架,原生 DOM 操作
- WebSocket 连接,token 通过 URL 参数或 localStorage 传递
- 所有静态资源 embed 到 `src/server/static/index.html`

---

## P32 实现:Web 客户端真实 API 接入

> 源文件: `/home/huanglin/Downloads/index.html` → 目标: `src/server/static/index.html`
> open-design 生成的 UI 使用 mock data,需清理注入并接入真实 API。

### 清理 open-design 注入

- [x] 删除 open-design 注入脚本 (sandbox-shim, tweaks-bridge-style, tweaks-bridge, snapshot-bridge, srcdoc-transport)
- [x] 删除所有 `data-od-id="..."` 属性
- [x] 清理后 index.html 从 44KB → 39KB

### 认证

- [x] 启动时 GET /api/sessions 测试认证,200 跳过登录,401 显示登录页
- [x] 登录页:token 存 localStorage('amux_token'),后续请求带 Bearer header
- [x] WebSocket 连接通过 ?token= query param 传 token

### Session 列表(替换 mock SESSIONS 数组)

- [x] loadSessions() 合并 GET /api/sessions + GET /api/ptys 结果
- [x] PTY 存活=running(绿), active=idle(黄), 其余=completed(灰)
- [x] setInterval(loadSessions, 10000) 每 10 秒刷新
- [x] renderSessions() 和 renderSidebar() 从 API 数据渲染

### 连接 Session(替换模拟 selectSession)

- [x] selectSession(id):关闭旧 WS,打开新 WS, binaryType=arraybuffer
- [x] onmessage: string → terminal.write(), ArrayBuffer → terminal.write(Uint8Array)
- [x] onclose/onerror: 状态点变红
- [x] 二进制帧 + 文本帧均已处理

### 发送输入(替换模拟 sendInput)

- [x] Input bar: ws.send(text + '\n'), xterm onData: ws.send(data)
- [x] 虚拟功能键: ESC/TAB/↑/↓/Ctrl+C/Ctrl+D 均通过 ws.send 发送正确转义序列
- [x] 所有键盘输入转发给 WS 而非 terminal.write

### 新建 Session(替换 mock push)

- [x] POST /api/sessions { agent, prompt }, header 带 auth
- [x] 成功后 loadSessions() + selectSession(data.id) 自动连接

### 保持不变

- [x] 所有 CSS(Tokyo Night 变量、响应式、横屏 landscape)
- [x] HTML 结构(登录页、主页面、新建弹窗、上下文菜单)
- [x] 触摸交互(滑动切换、长按菜单、下拉展开)
- [x] 双指缩放终端字体
- [x] 横屏 sidebar icon bar

### 验收

- [x] cargo build --release 编译通过(HTML 通过 include_str! embed)
- [x] cargo test 通过(170 tests)
- [x] 浏览器能看到登录页或 session 列表
- [x] 点击 session 能看到 PTY 实时输出
- [x] 输入栏和功能键能发送输入
- [x] 新建 session 弹窗能创建并自动连接
- [x] 手机竖屏/横屏布局正常

---

### P33: Agent 进程资源监控(通过 /proc)

> 当前完全看不到 agent 进程吃了多少 CPU/内存/IO。不需要 eBPF,用 Linux `/proc` 即可。
> `PtyHandle` 已持有 child PID(`portable-pty` 的 `ChildProcess::process_id()`),直接读 `/proc/{pid}/stat`。

- [x] `src/procfs.rs` 新模块 — 读取 /proc/{pid}/stat + /proc/{pid}/io + /proc/uptime
- [x] `ProcessStats` 结构体 — cpu_user, cpu_system, mem_rss_kb, mem_virt_kb, read_bytes, write_bytes, threads, cpu_percent
- [x] `read_process_stats(pid)` — 解析 /proc/{pid}/stat(处理 comm 中空格) + /proc/{pid}/io
- [x] `compute_cpu_percent()` — 两次采样 delta / elapsed * 100,缓存 prev values
- [x] `PtySlot` 增加 `process_stats: Option<ProcessStats>` 字段
- [x] `poll_states()` 中 30s 定时器采集每个活跃 PTY 进程统计
- [x] 采集频率 30s — 配合 last_stats_check Instant 定时器
- [x] TUI 侧边栏 — 活跃 PTY 行显示 CPU% + MEM (dim text)
- [x] TUI 状态栏 — 所有 PTY 汇总 CPU/MEM
- [x] Web API GET /api/ptys — 返回 cpu_percent, mem_rss_kb, mem_virt_kb, read_bytes, write_bytes, threads
- [x] 进程退出时 process_stats 保持最后快照
- [x] macOS 兼容 — cfg(target_os = "linux") 条件编译,非 Linux 返回空
- [x] 验收通过 — 3 个单元测试 + 173 tests + 0 clippy warnings