# 用 AI 二次开发

本项目的开发工作流基于 [web-dev-skills](https://github.com/timzaak/web-dev-skills)——一套独立的 AI 开发插件，把"决策 → PRD / 技术预研 → 设计 → 任务 → 开发 → 验收 → Demo"串成可恢复的流水线。工作流本身不内置在本仓库里——仓库只保留 agent 规则（`AGENTS.md`）和 `.ai/` 下的运行时产物。

如果你只是想给项目加个功能，按下面的步骤操作就行。不需要理解整套架构。

## 前置条件

- 一个已加载 web-dev-skills 插件的 AI 编程 agent（见下文）
- MCP Server `context7` 已配置（用于查询 Rust/React 第三方库文档）
- 项目已能正常编译运行（见[快速上手](getting-started.md)）

## 加载 web-dev-skills

web-dev-skills 是通用插件——同样的工作流可以直接用在其他 Rust + React 项目上。

- Claude Code：在目标项目里 `claude --plugin-dir /path/to/skills` 启动
- 其他 agent（ZCode、Codex 等）：在 `~/.agents/skills/` 下放置分发器 skill，把命令路由到克隆的插件仓库——见[在其他 AI 编程工具中使用 t-tools](https://github.com/timzaak/web-dev-skills/blob/main/human/use-in-other-agents.en.md)

完整安装说明：[timzaak/web-dev-skills](https://github.com/timzaak/web-dev-skills)。

## 最短闭环：加一个新功能

假设你要给产品加一个"固件版本管理"功能，功能名叫 `firmware-version`。

```shell
# 1. 生成 PRD 和用户故事草稿
t-prd firmware-version

# 2. 生成技术设计
t-design firmware-version

# 3. 生成任务并执行后端阶段
t-task firmware-version --phase backend
t-run firmware-version --phase backend

# 4. 前端阶段重复同样的闭环
t-task firmware-version --phase frontend
t-run firmware-version --phase frontend

# 5. 跑 E2E Demo 测试并验收
t-web-demo-run demo/e2e/firmware-version-demo.e2e.ts
t-web-demo-accept <role>

# 6. 验收通过后正式发布 PRD
t-prd-publish firmware-version
```

每条命令执行时会自动读取上一步的产物：`t-design` 读 `t-prd` 生成的 PRD，`t-run` 读 `t-task` 生成的任务计划。

命令的调用方式随 agent 而异：Claude Code 里是斜杠命令（`/t-prd`）；不支持插件的 agent 通过分发器调用（`/t-tool t-prd`）。

`t-prd-check`、`t-design-check`、`t-task-check` 是可选的质量关卡——某一阶段产物感觉不稳时按风险使用。

## 各命令说明

### 需求和设计阶段

| 命令 | 参数 | 产物位置 | 干什么 |
|------|------|----------|--------|
| `t-decision <name>` | 功能名 | `.ai/decision/` | 产品立项判断，按主要未知项进入预研或 PRD |
| `t-tech-research <name>` | 主题 | `.ai/tech-research/` | 设计前的可行性 / 依赖 / 成本预研 |
| `t-prd <name>` | 功能名 | `.ai/prd/`、`.ai/user-stories/` | 生成或更新 PRD 和用户故事草稿 |
| `t-prd-check <name>` | 功能名 | `.ai/quality/` | 检查 PRD 完整性和用户故事质量 |
| `t-prd-publish <name>` | 功能名 | `docs/prd/` | 验收通过后正式发布 PRD |
| `t-design <name>` | 功能名 | `.ai/design/` | 生成技术设计（API、数据库 schema、实现方案） |
| `t-design-check <name>` | 功能名 | `.ai/quality/` | 评估设计可实施性，打分 |

### 任务和执行阶段

| 命令 | 参数 | 产物位置 | 干什么 |
|------|------|----------|--------|
| `t-task <name> --phase <backend\|frontend>` | 功能名 + 阶段 | `.ai/task/<name>/` | 把设计拆成该阶段可执行的任务 |
| `t-task-check <name>` | 功能名 | `.ai/quality/` | 检查任务计划可执行性 |
| `t-run <name> --phase <backend\|frontend>` | 功能名 + 阶段 | `.ai/task/<name>/` | 按阶段驱动 sub-agent 执行开发 |
| `t-super-run <name> --phase <phase>` | 功能名 + 阶段 | `.ai/super-run/` | 强模型单主会话路径，合并规划与执行 |

### 测试和验收阶段

| 命令 | 参数 | 产物位置 | 干什么 |
|------|------|----------|--------|
| `t-web-demo-run <file>` | `demo/e2e/` 下的路径，如 `demo/e2e/alarm-rules-demo.e2e.ts` | `.ai/quality/` | 跑单个 Playwright E2E 测试 |
| `t-web-demo-run-all` | 可选 `continue` | `.ai/quality/` | 跑全部 E2E 测试 |
| `t-web-demo-accept <role>` | 角色名 | `.ai/quality/` | 验收测试覆盖度和可运行性 |

web-dev-skills 还覆盖本项目用不到的技术栈（Figma、小程序、Flutter）——完整命令列表见插件 README。

## 实操建议

### 分阶段跑，不要一次跑完

先跑后端，再跑前端：前端依赖后端的 OpenAPI schema。后端有变更后，重新生成：

```shell
cd frontend && npm run generate-api
```

### PRD 可以迭代更新

跑完 `t-prd` 之后如果你发现需求有遗漏，直接再跑一次 `t-prd`，它会读取已有的 PRD 文件做增量更新，不会覆盖你手动改的内容。

### Demo 测试失败时的处理

用 `t-web-demo-run` 重跑失败的测试，它会自动诊断并尝试修复。如果自动修复不了，看 `.ai/quality/` 下的诊断报告——报告会告诉你失败原因是前端问题、后端问题、还是测试本身的问题。

## 文件位置速查

```
AGENTS.md              本仓库的 agent 规则
.ai/prd/               PRD 草稿（经 t-prd-publish 发布到 docs/prd/）
.ai/user-stories/      用户故事草稿
.ai/design/            技术设计文档
.ai/task/              任务计划和执行状态
.ai/quality/           各类检查和验收报告
docs/prd/              正式发布的 PRD
demo/e2e/              Playwright E2E 测试
frontend/api.json      后端导出的 OpenAPI schema
```

## 只想改个小 bug

小修小补不需要走流水线。直接向你的 AI agent 描述要改什么，它会遵循 `AGENTS.md` 里的规则来辅助你。
