# Outcome

将本仓库（Pumpkin-MC/Pumpkin 的生电特性 fork，remote 已指向 VerdantGolemMC/VerdantGolem）的项目品牌从 Pumpkin 改名为 Verdantgolem 的"项目自描述层"：crate/目录/包名/二进制名、全部 use 导入路径、面向人的显示字符串（MOTD、banner、brand、crash 报告、`/verdantgolem` 命令）、文档与部署脚本。所有与上游 Pumpkin 的线上/磁盘契约、游戏内容和内部 Rust 标识符保持不变，以最小化未来同步上游的冲突。改名后项目可正常构建、测试。

# Scope

## 改名对象（Pumpkin → VerdantGolem / pumpkin → verdantgolem）

- Cargo workspace：`crates/pumpkin*` 14 个目录 + `tools/pumpkin-{codegen,fuzzer}` 2 个目录改名为 `verdantgolem*`；所有 package name、依赖键、path、根 Cargo.toml members 与 workspace.dependencies；两个 fuzz 子 crate（pumpkin-nbt/fuzz、pumpkin-protocol/fuzz）；`default-run`、tauri-winres 元数据（FileDescription/OriginalFilename）。
- 导入与路径：全部 `use pumpkin*::` / 全限定路径（约 11,000 行）、`extern crate pumpkin_macros`、宏生成代码中的 crate 路径（api-macros 生成的 `pumpkin::plugin::...` → `verdantgolem::plugin::...`、macros 生成的 `pumpkin_util/pumpkin_data` 引用）、`translate_cross` 宏的路径段匹配表（`pumpkin_data` → `verdantgolem_data`，防止翻译校验静默失效）。
- codegen：`tools/verdantgolem-codegen` 的 `OUT_DIR` 改指 `crates/verdantgolem-data/src/generated`、`MAPPING_OUT_DIR` 改指主 crate 新路径（`WIT_OUT_DIR` 不变，仍指向保留的 pumpkin-plugin-wit 子模块）；改名后重新生成全部 generated 文件。
- 运行时显示品牌：server brand（connection_cache BRAND）、默认 MOTD（java/bedrock 配置与 connection_cache）、启动 banner 与 under development 提示、crash 报告文本（"Pumpkin has encountered a panic!"、"Pumpkin Crash Report"、"Pumpkin Version:" 标签）、默认 level_name "VerdantGolem world"、server_version "VerdantGolem Rust Server"、插件加载器错误文案、bedrock status 测试 MOTD。
- 命令：`/pumpkin` → `/verdantgolem`（文件 pumpkin.rs → verdantgolem.rs，NAMES=["verdantgolem","version","ver"]），命令树注册键与 dispatcher 测试同步；GitHub API/issue 链接指向 VerdantGolemMC/VerdantGolem；翻译键名 `commands.pumpkin.*` 保留，25 个语言文件的值改为 VerdantGolem（值内的 GitHub 链接指向新仓库）。
- 文档与元数据：README.md（标题、badge、正文重写为 VerdantGolem 生电分支并保留 fork 出处）、CONTRIBUTING.md、assets/NOTICE.md（保留上游出处表述，更新已改名的 crate 路径）、.devcontainer 名称、约 168 处 doc 注释中指本项目的品牌词（逐词甄别，不碰游戏内容）、assets/pumpkin-chunk-loading.webp 文件名与引用。
- 构建与部署：Dockerfile（WORKDIR、-p、二进制路径、ENTRYPOINT）、docker-compose.yml（service 名、容器内路径、镜像注释）、egg-pumpkin.json → egg-verdantgolem.json（startup、REPO_URL、cd VerdantGolem、二进制名）、flake.nix（importTOML 路径、--package）、typos.toml（排除路径改指 verdantgolem-data，并加 verdantgolem 词）、.github/（rust.yml artifact/dist 名、docker.yml 仓库守卫改为 VerdantGolemMC/VerdantGolem 与 PUMPKIN_VERSION 变量名、CODEOWNERS 路径、ISSUE_TEMPLATE "/pumpkin" 提示、PR 模板链接）。
- Cargo.lock（根 + codegen 目录）由 cargo 构建时增量重写，不手改不整删。

## 保留对象（不改，上游契约与同步友好）

- 游戏内容：`minecraft:pumpkin*`、`PUMPKIN_STEM/CARVED_PUMPKIN/JACK_O_LANTERN/PUMPKIN_PIE/PUMPKIN_SEEDS` 枚举、`PumpkinBlock/CarvedPumpkinBlock` 游戏 struct、`BlockPumpkinCarve`、`block.pumpkin.carve`、datapack/资产文件中全部南瓜 worldgen/recipe/loot/翻译（约 507 行）。
- 持久化/线上契约（用户决策：上游是 pumpkin，方便同步）：NBT 键 `"PumpkinCustomData"`、世界文件 `pumpkin_custom_data.nbt`、配置文件 `pumpkin.toml`（.gitignore/.dockerignore 相应行不变）、`pumpkin:` 命名空间值（PUMPKIN_NAMESPACE、recipe_id、皮肤 id、`pumpkin:enchantments`、翻译注册 `pumpkin:{key}`、命令树命名空间列表）、日志 target `"pumpkin_plugin"`。
- 插件接口（用户决策：保持 pumpkin 命名空间）：WIT 命名空间 `pumpkin:plugin@0.1.0` 与全部 `wit::v0_1::pumpkin::plugin` 引用、ABI 符号 `PUMPKIN_API_VERSION`（生成端与 native 加载端都不动）、wasm section `pumpkin.metadata`/`pumpkin.signature`、User-Agent `"Pumpkin-MC"`、`market.pumpkinmc.org` 全部 URL、子模块 `crates/pumpkin-plugin-wit`（目录名、URL、.gitmodules 及所有指向它的相对路径引用均不变）。
- 内部 Rust 标识符（降低上游同步冲突，用户决策精神的延伸）：`PumpkinServer/PumpkinConfig/PumpkinError/PumpkinCommandCompleter/PumpkinMetadata/PumpkinBlock` 等类型名、宏名 `pumpkin_block/pumpkin_block_from_tag`、`get_pumpkin_block` 访问器、`PUMPKIN_API_VERSION/PUMPKIN_METADATA_SECTION/PUMPKIN_EN_US_JSON`（含 PUMPKING_IT_IT_JSON 原拼写错误）等常量/静态名。
- 上游社区链接：Discord 邀请、pumpkinmc.org/donate、FUNDING.yml、doc 注释中指向 docs.pumpkinmc.org 的协议文档链接（作为出处保留）。

# Non-goals

- 不实现生电特性本身（本 change 只做品牌改名，生电特性是后续工作）。
- 不变更任何游戏逻辑、协议行为与功能语义（品牌字符串与标识符除外）。
- 不做旧 Pumpkin 存档/配置的迁移或读取兼容改造（键名原样保留，天然兼容）。
- 不拆分为 Supervisor Change：改名是紧耦合的单一机械操作，只有全部完成后才能编译通过，无独立可验证的并行价值。

# Acceptance examples

- A1: `git submodule update --init crates/pumpkin-plugin-wit` 后 `cargo build --release` 成功，产物二进制名为 `verdantgolem`，workspace 无 pumpkin* 目录。
- A2: `cargo nextest run` 与 `cargo test --doc`（CI 同款）全部通过。
- A3: `cargo clippy --all-targets --all-features`（CI 同款）无新增错误。
- A4: 品牌残留扫描为零（白名单除外）：crate 名/导入/品牌字符串/URL/文档标题不再有品牌类 pumpkin token；白名单 = 游戏内容 token、保留的契约标识（PumpkinCustomData、pumpkin.toml、pumpkin: 命名空间、pumpkin.metadata 等）、保留的内部标识符（PumpkinServer 等类型/宏/常量名、pumpkin_plugin 日志 target）、子模块路径与上游社区/市场 URL、NOTICE/README 中的出处表述。
- A5: 运行时品牌正确：BRAND 常量为 "VerdantGolem"、默认 MOTD、banner、crash 报告文本、`/verdantgolem` 命令输出均为新品牌；`/pumpkin` 不再注册。
- A6: codegen 三个输出路径常量与新目录一致；`crates/verdantgolem-data/src/generated` 与主 crate 的 WIT mapping 生成文件引用新 crate 名（`use verdantgolem_util::` 等），游戏枚举段与改名前 bit 级一致（git diff 仅导入行变化）。
- A7: CI/部署无失效引用：rust.yml artifact 名与二进制路径、docker.yml 守卫仓库、Dockerfile、compose、egg-verdantgolem.json、flake.nix、typos.toml 排除路径、CODEOWNERS 路径全部指向新命名。
- A8: 保留契约零回归：`PumpkinCustomData`、`pumpkin_custom_data.nbt`、`pumpkin.toml`、`PUMPKIN_NAMESPACE = "pumpkin"`、WIT `pumpkin:plugin`、`PUMPKIN_API_VERSION`、wasm section 名在代码中与改名前一致（grep 逐项核对）。

# Constraints and invariants

- 大小写敏感、按模式替换：只改 `pumpkin_`/`pumpkin::`/`"pumpkin:`/crate 名/品牌词 `Pumpkin`（限指项目的语境），绝不触碰白名单 token；替换后对剩余大写 `Pumpkin` 逐处人工复核（防句首游戏内容误伤）。
- `translate_cross` 宏路径段匹配表必须与调用点同步改为 `verdantgolem_data`，否则翻译参数校验静默失效。
- api-macros 生成代码的 crate 路径、macros 生成代码的 crate 路径必须与改名后的 crate 名一致（`PUMPKIN_API_VERSION` 符号本身不动）。
- 生成文件必须通过运行 codegen 重新生成（codegen 无法运行时允许等价文本替换，但需在 handoff 中记录原因）。
- GPLv3 与 plugin-api 的 MIT/Apache 双许可出处义务保留（NOTICE.md/README 出处表述、LICENSE 不动）。

# Decisions

- D1（Q1，用户选定）：crate/目录/二进制命名采用 verdantgolem 一体式（crates/verdantgolem、verdantgolem-config、二进制 verdantgolem）。
- D2（Q2，用户补充"以后上游是 pumpkin 方便同步"）：存档/配置/命名空间等持久化与线上契约标识全部保留 pumpkin 原名，不做改名也不做迁移，天然兼容旧数据并减少上游格式变更的同步冲突。
- D3（Q3，用户选定）：插件接口保持 pumpkin 命名空间（WIT/ABI 符号/wasm section/User-Agent/market URL），子模块 crates/pumpkin-plugin-wit 保持原路径原 URL，其全部引用不变。
- D4（Q4，用户选定）：文档保留 fork 出处——README 声明 VerdantGolem 是 Pumpkin 的生电特性分支，LICENSE/NOTICE 保留上游版权出处，其余品牌改写。
- D5：内部 Rust 标识符（类型名、宏名、常量/静态/函数名、日志 target）保留原名，是 D2/D3 同步友好原则的延伸；改名的深度止步于 crate 路径与显示字符串。
- D6：`/pumpkin` 命令改为 `/verdantgolem`（用户可见品牌），翻译键名保留仅改值；GitHub API/链接指向 VerdantGolemMC/VerdantGolem；上游社区（Discord/捐赠）与市场 URL 保留。
- D7：Cargo.lock 不手改、不整删，构建时由 cargo 增量重写，避免外部依赖升级。
- D8：单一 Native change，不拆分 Supervisor Change（紧耦合、无并行验证价值）。
- 工作区：使用当前目录（master 分支）。

# Open questions

（无——用户已于 2026-08-22 确认目标、范围、关键决定 D1-D8、验收 A1-A8 与非目标。）

# Verification expectations

- 开发期检查（CI 同款）：`cargo build --release`、`cargo nextest run`、`cargo test --doc`、`cargo clippy --all-targets --all-features`、`cargo fmt --check`。
- 残留扫描：品牌类 pumpkin token 全仓检索为零（白名单见 A4）；A8 契约清单逐项 grep 核对不变。
- 生成文件核对：git diff 确认 generated 文件仅导入路径变化，游戏枚举值无改动。
- 运行时冒烟（环境允许时）：启动服务器确认 banner/MOTD/`/verdantgolem` 输出。
