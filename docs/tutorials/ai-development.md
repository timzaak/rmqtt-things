# 用 Claude Code 二次开发

项目内置了一套基于 Claude Code 的 AI 开发工作流。`.claude/` 目录下有 skill、agent、guide、protocol 四层配置，把"需求 → 设计 → 开发 → 验收"串成可复用的流水线。

如果你只是想给项目加个功能，按下面的步骤操作就行。不需要理解整套架构。

## 前置条件

- [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code) 已安装并登录
- MCP Server `context7` 已配置（用于查询 Rust/React 第三方库文档）
- 项目已能正常编译运行（见[快速上手](getting-started.md)）

## 通用 AI 开发配置

项目内的 `.claude/` 配置是针对 RMQTT Things 定制的。如果你要在自己的 Rust + React 项目里用同样的工作流，看这个独立项目：

**[timzaak/web-dev-skills](https://github.com/timzaak/web-dev-skills)** — 通用的 Claude Code 插件，支持 PRD 到 Demo 的完整链路。

安装方式：

```shell
git clone https://github.com/timzaak/web-dev-skills.git
cd /your-project
claude --plugin-dir /path/to/web-dev-skills
```

## 最短闭环：加一个新功能

假设你要给产品加一个"固件版本管理"功能，功能名叫 `firmware-version`。

```shell
# 1. 写 PRD
/t-prd firmware-version

# 2. 检查 PRD 质量（不要跳过）
/t-prd-check firmware-version

# 3. 生成技术设计
/t-design firmware-version

# 4. 检查设计（不要跳过）
/t-design-check firmware-version

# 5. 生成任务计划
/t-task firmware-version

# 6. 检查任务计划
/t-task-check firmware-version

# 7. 执行后端开发
/t-run firmware-version --phase backend

# 8. 后端收口（格式化、clippy、测试、导出 OpenAPI）
/t-backend-finalize firmware-version

# 9. 执行前端开发
/t-run firmware-version --phase frontend

# 10. 跑 Demo 测试并验收
/t-demo-run firmware-version
/t-demo-accept firmware-version
```

每条命令执行时会自动读取上一步的产物。比如 `/t-design` 会读 `/t-prd` 生成的 PRD 文件，`/t-run` 会读 `/t-task` 生成的任务计划。

## 各命令说明

### 需求和设计阶段

| 命令 | 参数 | 产物位置 | 干什么 |
|------|------|----------|--------|
| `/t-prd <name>` | 功能名 | `docs/prd/<domain>/<name>.md` | 生成或更新 PRD 文档和用户故事 |
| `/t-prd-check <name>` | 功能名 | `.ai/quality/prd-check-*.md` | 检查 PRD 完整性和用户故事质量 |
| `/t-design <name>` | 功能名 | `.ai/design/<name>.md` | 生成 API 设计、数据库 schema、实现方案 |
| `/t-design-check <name>` | 功能名 | `.ai/quality/design-check-*.md` | 评估设计可实施性，打分 |
| `/t-consistency-check <module>` | 模块名或 `--all` | `.ai/quality/consistency-*.md` | 对比 PRD 和实现，找差距 |

### 任务和执行阶段

| 命令 | 参数 | 产物位置 | 干什么 |
|------|------|----------|--------|
| `/t-task <name>` | 功能名 | `.ai/task/<name>/` | 把设计拆成可执行任务 |
| `/t-task-check <name>` | 功能名 | `.ai/quality/task-check-*.md` | 检查任务计划可执行性 |
| `/t-run <name> --phase <backend\|frontend\|demo>` | 功能名 + 阶段 | `.ai/task/<name>/.state.json` | 按阶段驱动 sub-agent 执行开发 |
| `/t-backend-finalize <name>` | 功能名 | 更新 `frontend/api.json` | 格式化、clippy、跑测试、导出 OpenAPI |

### 测试阶段

| 命令 | 参数 | 产物位置 | 干什么 |
|------|------|----------|--------|
| `/t-backend-test-run` | 无 | 控制台输出 | 跑 Rust 测试，失败自动诊断修复 |
| `/t-demo-run <file>` | 测试文件路径或角色名 | `.ai/quality/demo-run-*.md` | 跑单个 E2E 测试，失败自动修复 |
| `/t-demo-run-all` | 可选 `continue` | `.ai/quality/demo-run-all-*.md` | 跑全部 E2E 测试 |
| `/t-demo-accept <name>` | 测试路径、角色名或 `all` | `.ai/quality/demo-accept-*.md` | 验收测试覆盖度和可运行性 |

## 实操建议

### 分阶段跑，不要一次跑完

`/t-run` 支持 `--phase` 参数，按阶段执行：

```shell
# 先跑后端，确认编译通过
/t-run firmware-version --phase backend

# 再跑前端
/t-run firmware-version --phase frontend

# 最后跑 Demo
/t-run firmware-version --phase demo
```

后端阶段跑完后再跑前端，因为前端依赖后端的 OpenAPI schema。后端收口命令 `/t-backend-finalize` 会把最新的 `api.json` 导出到 `frontend/` 目录。

### 不要跳过 check 命令

`/t-prd-check`、`/t-design-check`、`/t-task-check` 不是装饰。它们在产物质量不达标时会给出具体的修复建议。跳过这些检查，问题会累积到后面更难修。

### PRD 可以迭代更新

跑完 `/t-prd` 之后如果你发现需求有遗漏，直接再跑一次 `/t-prd`，它会读取已有的 PRD 文件做增量更新，不会覆盖你手动改的内容。

### Demo 测试失败时的处理

```shell
# 跑单个测试，自动诊断和修复
/t-demo-run firmware-version

# 如果自动修复不了，看诊断报告
# 报告在 .ai/quality/demo-run-*.md
```

诊断报告会告诉你失败原因是前端问题、后端问题、还是测试本身的问题，并给出修复建议。

## 文件位置速查

```
docs/prd/              PRD 文档
.ai/design/            技术设计文档
.ai/task/              任务计划和执行状态
.ai/quality/           各类检查和验收报告
frontend/api.json      后端导出的 OpenAPI schema
```

## 只想改个小 bug

如果你只是修个 bug 或加个小改动，不需要走完整流水线。直接在 Claude Code 里描述你要改什么，它会根据 `CLAUDE.md` 和 `.claude/` 里的配置来辅助你。

对于 bug 修复，建议的操作：

```shell
# 如果是后端 bug
/t-backend-test-run    # 先确认现有测试通过

# 改完代码后
/t-backend-finalize <name>  # 格式化、clippy、测试
```
