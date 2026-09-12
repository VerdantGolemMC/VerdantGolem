# 上游自动同步

VerdantGolem 跟踪 [Pumpkin-MC/Pumpkin](https://github.com/Pumpkin-MC/Pumpkin) 的 `master`。
入口为 `.github/workflows/upstream-sync.yml`，准备器为 `.github/sync/upstream_sync.py`，
发布与 CI 门禁为 `.github/sync/publish_sync.py`。

## 触发与配置

- 每日 **03:00 UTC / 北京时间 11:00** 检查；GitHub 的定时任务可能排队延后。
- `master` 有新推送时也检查；已包含最新上游提交时直接结束。
- Actions 页面可手动运行。`dry_run` 默认勾选，只生成报告；取消勾选后才发布同步 PR。
  手动预演使用所选分支的提交，因此可以在功能 PR 合入前验证当前改动；正式同步始终以 `master` 为基线。
- `auto_merge` 默认开启；手动取消，或把仓库变量 `UPSTREAM_AUTO_MERGE` 设为 `false`，
  即可保留已验证的 PR 等待人工合并。
- 自动分支使用 **`codex/upstream-sync`**，复用同一个 open PR，并保留自动正文之外的人工备注。
  旧的 `auto/upstream-sync` 分支不自动删除或覆盖。

工作流必须合入默认分支才能按计划运行。仓库还需在 **Settings → Actions → General** 开启
**Allow GitHub Actions to create and approve pull requests**。默认 workflow token 权限保持 **Read**；
写权限只授予发布 job。这里不要求 PAT，也不依赖仓库的 “Allow auto-merge” 开关。
仓库需允许 merge commit，以保留上游父提交；额外的审批、签名和分支保护规则仍由 GitHub 执行。

## 合并方式：先统一命名空间，再做三方合并

简单降低 `git merge` 的改名相似度会制造大量路径/格式冲突，也可能遗留 `pumpkin-*` 孤儿目录。
新准备器依次执行：

1. 固定当前 fork 和上游 SHA，检查共同祖先与已记录的上游状态。
2. 将共同祖先、上游树里的 crate 路径和精确 crate 标识映射到 `verdantgolem-*`。
   新增的 command、GameTest、bindings 等 crate 按目录自动发现，不依赖固定旧名单。
3. 用同一份 fork rustfmt 配置格式化三方 Rust 内容，消除改名导致的换行差异；不做全文件小写比较。
4. 通过 `git merge-tree` 三方合并。非重叠修改自动保留两侧；真正重叠的生电实现进入冲突报告。
5. 检查结果中没有旧 crate 路径。`Cargo.lock` 以规范化后的上游版本为起点，
   在隔离工作树运行 `cargo update --workspace`，补齐 fork 自有依赖和 workspace 包名。
   先尝试离线，缺包时再正常解析；锁文件在候选提交产生前刷新，而不是期待 `--locked` 构建修改它。
6. 干净结果生成确定性的候选 merge commit，父提交为 **原 fork SHA + 原上游 SHA**。
   `.github/sync/upstream-state.json` 记录上游位置；树、父提交和解析后的锁文件不变时，
   重复运行得到同一候选 SHA，不会仅因执行时间不同而重推。

准备过程不 checkout 候选到调用者工作区，也不修改调用者 index；临时 worktree/index 使用独立目录。
历史被改写、状态损坏、命名映射碰撞或真实冲突都会输出错误，而不是猜测另一个合并基线。

### 保留边界

- `.github/`、品牌/部署文件、项目与生电文档、`.gitignore` 和 `rustfmt.toml` 保留 fork 版本，
  即使原始 Git 合并本来没有冲突。完整名单在准备器的 `OWNED_FILES` / `OWNED_PREFIXES`。
- `crates/pumpkin-plugin-wit` gitlink 跟踪上游；它不是需要改名的 Rust crate。
- `pumpkin:plugin`、WIT 的 `pumpkin::plugin`、`PUMPKIN_API_VERSION`、`PumpkinCustomData`、
  `pumpkin_block*`、`PumpkinServer` 及 `pumpkin.toml` 等兼容标识保留。
- API macro 中指向实际 Rust 主 crate 的模板路径仍映射到 `verdantgolem`。
- 二进制文件和 executable/gitlink 模式保留；新增、删除、改名都参与正常三方判断。

## CI 和发布门禁

准备 job 只有 `contents: read`，checkout 不保留凭据；Cargo 子进程不接收发布 token。
候选通过 Git bundle artifact 传给另一 job。发布 job 始终执行原工作流的可信脚本，
不执行候选代码；它重新核验父提交、tree、上游状态，以及 fork-owned 文件与 base 完全一致。

发布 job 使用 `GITHUB_TOKEN` 推送自动分支并创建/更新 PR。只对自动分支使用
`--force-with-lease`；人工改过的 PR head 会停止更新。`master` 通过普通 PR 合并接口更新，
没有 `--admin` 或分支保护覆盖。

GITHUB_TOKEN 产生的 push/PR 事件不作为触发 CI 的保证，因此显式 dispatch：

- `rust.yml`：格式、依赖检查、debug/release Clippy、Linux/macOS/Windows/Android 构建与测试、
  同步脚本回归，以及临时真实服务器的生电 smoke。
- `typos.yml`：拼写检查。
- `nix.yml`：Flake 校验与构建。

每轮使用唯一 `sync_run_id`。门禁同时匹配 **workflow、event、branch、head SHA 和 run-name**，
还检查必要 job/matrix 确实执行成功。空检查、旧提交的绿灯、跳过、取消、失败和超时都不会触发合并。
更改 CI job 名称或矩阵时，需要同步更新 `publish_sync.py` 的 `REQUIRED_JOBS` 合约。

合并前再次核对 base/head、上游提交仍属于配置分支的历史，以及 GitHub mergeability；
base 已推进或上游改写了目标历史时重新准备，而不是合并旧验证结果。
成功合并后显式触发 `master` 的 Rust workflow（`publish_nightly=true`）和 Docker workflow，
避免 token 事件抑制导致 Nightly/镜像一直停留在旧提交。Nightly 仍依赖所有构建和生电 smoke 成功。

## 冲突与失败处理

仓库禁用了 Issues，因此通过 Actions summary 和 artifact 保留现场：

- `report.json` / `summary.md`：固定 SHA、提交数量、状态与冲突路径。
- `merge.patch` / `merge-details.txt`：仅在有冲突时用于审查的合并现场，可能含冲突标记。
- `cargo-lock.log` / `rustfmt.log`：准备失败原因。
- `publication.json`：PR、精确 CI run URL、合并或发布结果。

真实冲突时不生成可发布候选，不推送半合并分支。应在独立集成分支中按最新上游 API 移植
Carpet 接入点并验证；保留原上游父提交，避免 squash/rebase 丢失跟踪关系。
不要把带冲突标记的 patch 直接提交，也不要通过整文件取上游来让生电实现消失。
基线修复合入后，后续 push/定时运行会继续自动同步。

CI 失败或需要额外审批时 PR 保持打开并附失败信息；若已合并但发布 dispatch 失败，
结果标记为 `merged_publication_failed`，可手动运行 master 的发布工作流。

## GitHub 上的预演与回归

本次开发后续的构建、测试、lint 与同步预演统一在 GitHub Actions 执行。
本地只编辑代码、提交和推送；以下入口使用已推送分支：

```sh
gh workflow run rust.yml --ref codex/technical-survival-completion
gh workflow run typos.yml --ref codex/technical-survival-completion
gh workflow run nix.yml --ref codex/technical-survival-completion
gh workflow run upstream-sync.yml --ref codex/technical-survival-completion -f dry_run=true -f auto_merge=false
gh run list --branch codex/technical-survival-completion
```

所有同步脚本回归与真实服务器 smoke 都已接入 Rust workflow。预演的冲突报告可在相应 run 的 artifact 下载。

### 准备器命令参考

工具：Git 2.50+、Python 3.9+、项目 stable Rust/rustfmt。

```sh
python3 -m unittest discover -s .github/sync -p 'test_*.py' -v
actionlint
git fetch --no-tags upstream master
python3 .github/sync/upstream_sync.py \
  --head HEAD --upstream upstream/master --format-rust \
  --output target/verification/upstream-preview
```

预演默认只读取指定提交，不自动暂存未提交文件。`--refresh-lock` 可额外验证合并后的 lockfile；
`--bundle` 生成供发布器核验的候选对象包。这些选项都不推送远端。

切换为 GH 验证前的本地历史预演固定到 `5bbbf4da847e02c4194cbd4867404eea5f103cf1`：
相对远端当前 `c52839ee3` 有 141 个新提交；仅做路径转换会报告 158 个冲突，统一 Rust 格式后
剩下 24 个实际重叠点。它们集中在命令系统迁移、实体/区块和生电规则接入，按上述报告策略保留。
这说明自动机制已经消除了机械冲突，不代表这批历史积压已经合入。
另一次包含本轮尚未提交的假人、物品交互等实现的隔离快照预演报告了 60 个重叠路径；
实际启用时应以当次 base 的报告为准。两次预演均未改动真实 HEAD、index 或工作区内容。
