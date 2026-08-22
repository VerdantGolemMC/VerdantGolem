---
generated_from_state_version: 8
---

# Verification

## Current result

- Result: **Passed, user confirmation required**
- Assurance: **skill-coordinated**
- Goal cycle: 1
- Iteration: 2
- Verifier attempt: 1
- Completed: 2026-08-22T11:46:34.867Z
- Summary: 第 2 轮复核通过（49/49）。f490fec76 与声称一致：3 文件 4 行注释级改动，A4 三处残留清除，全仓抽样无新问题，其余 48 项维持第 1 轮通过结论。verdict: pass。

## Acceptance

| ID | Result | Source | Criterion | Reason |
| --- | --- | --- | --- | --- |
| A1 | passed | brief.md | A1: `git submodule update --init crates/pumpkin-plugin-wit` 后 `cargo build --release` 成功，产物二进制名为 `verdantgolem`，workspace 无 pumpkin* 目录。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A2 | passed | brief.md | A2: `cargo nextest run` 与 `cargo test --doc`（CI 同款）全部通过。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A3 | passed | brief.md | A3: `cargo clippy --all-targets --all-features`（CI 同款）无新增错误。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A4 | passed | brief.md | A4: 品牌残留扫描为零（白名单除外）：crate 名/导入/品牌字符串/URL/文档标题不再有品牌类 pumpkin token；白名单 = 游戏内容 token、保留的契约标识（PumpkinCustomData、pumpkin.toml、pumpkin: 命名空间、pumpkin.metadata 等）、保留的内部标识符（PumpkinServer 等类型/宏/常量名、pumpkin_plugin 日志 target）、子模块路径与上游社区/市场 URL、NOTICE/README 中的出处表述。 | 本轮实测通过：git show f490fec76 确认 3 文件 4 行注释级修复与声称一致；git grep 复核三处旧残留（pumpkin-plugin-utils、'use pumpkin random'、pumpkin's Xoroshiro）代码中零命中；全仓抽样（包名/目录/导入/旧 crate 名模式/TODO 注释）均为零或白名单类 |
| A5 | passed | brief.md | A5: 运行时品牌正确：BRAND 常量为 "VerdantGolem"、默认 MOTD、banner、crash 报告文本、`/verdantgolem` 命令输出均为新品牌；`/pumpkin` 不再注册。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A6 | passed | brief.md | A6: codegen 三个输出路径常量与新目录一致；`crates/verdantgolem-data/src/generated` 与主 crate 的 WIT mapping 生成文件引用新 crate 名（`use verdantgolem_util::` 等），游戏枚举段与改名前 bit 级一致（git diff 仅导入行变化）。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A7 | passed | brief.md | A7: CI/部署无失效引用：rust.yml artifact 名与二进制路径、docker.yml 守卫仓库、Dockerfile、compose、egg-verdantgolem.json、flake.nix、typos.toml 排除路径、CODEOWNERS 路径全部指向新命名。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A8 | passed | brief.md | A8: 保留契约零回归：`PumpkinCustomData`、`pumpkin_custom_data.nbt`、`pumpkin.toml`、`PUMPKIN_NAMESPACE = "pumpkin"`、WIT `pumpkin:plugin`、`PUMPKIN_API_VERSION`、wasm section 名在代码中与改名前一致（grep 逐项核对）。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A9 | passed | specs/project-identity/spec.md | 本 capability 定义 VerdantGolem 仓库（Pumpkin-MC/Pumpkin 的生电特性 fork）改名后的完整命名体系：哪些标识属于 VerdantGolem 项目自描述层（改名），哪些属于上游契约、游戏内容或同步友好保留层（不改名）。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A10 | passed | specs/project-identity/spec.md | 品牌名：`VerdantGolem`（显示）、`verdantgolem`（代码/包名，一体式小写，无连字符无缩写）、`VERDANTGOLEM`（如需大写环境变量）。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A11 | passed | specs/project-identity/spec.md | \| 旧 \| 新 \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A12 | passed | specs/project-identity/spec.md | \| crates/pumpkin \| crates/verdantgolem \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A13 | passed | specs/project-identity/spec.md | \| crates/pumpkin-api-macros \| crates/verdantgolem-api-macros \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A14 | passed | specs/project-identity/spec.md | \| crates/pumpkin-codecs \| crates/verdantgolem-codecs \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A15 | passed | specs/project-identity/spec.md | \| crates/pumpkin-config \| crates/verdantgolem-config \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A16 | passed | specs/project-identity/spec.md | \| crates/pumpkin-data \| crates/verdantgolem-data \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A17 | passed | specs/project-identity/spec.md | \| crates/pumpkin-inventory \| crates/verdantgolem-inventory \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A18 | passed | specs/project-identity/spec.md | \| crates/pumpkin-macros \| crates/verdantgolem-macros \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A19 | passed | specs/project-identity/spec.md | \| crates/pumpkin-nbt \| crates/verdantgolem-nbt \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A20 | passed | specs/project-identity/spec.md | \| crates/pumpkin-plugin-api \| crates/verdantgolem-plugin-api \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A21 | passed | specs/project-identity/spec.md | \| crates/pumpkin-plugin-utils \| crates/verdantgolem-plugin-utils \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A22 | passed | specs/project-identity/spec.md | \| crates/pumpkin-protocol \| crates/verdantgolem-protocol \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A23 | passed | specs/project-identity/spec.md | \| crates/pumpkin-util \| crates/verdantgolem-util \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A24 | passed | specs/project-identity/spec.md | \| crates/pumpkin-world \| crates/verdantgolem-world \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A25 | passed | specs/project-identity/spec.md | \| tools/pumpkin-codegen \| tools/verdantgolem-codegen \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A26 | passed | specs/project-identity/spec.md | \| tools/pumpkin-fuzzer \| tools/verdantgolem-fuzzer \| | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A27 | passed | specs/project-identity/spec.md | package name、依赖键、`[workspace.dependencies]` path、`default-run`、fuzz 子 crate（verdantgolem-nbt-fuzz、verdantgolem-protocol-fuzz）与各自 Cargo.lock 条目同步。导入路径统一变为 `use verdantgolem*::`（连字符包名在 use 中即 `verdantgolem_util` 形式）。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A28 | passed | specs/project-identity/spec.md | `crates/pumpkin-plugin-wit`：git 子模块，路径与 URL（Pumpkin-MC/pumpkin-plugin-wit）不变，`.gitmodules` 及所有相对路径引用（bindgen!/wit_bindgen!、cargo-component metadata target、codegen WIT_OUT_DIR）不变。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮已 ls/git grep 复核目录与包名） |
| A29 | passed | specs/project-identity/spec.md | **构建产物**：二进制名 `verdantgolem`；Windows 资源 FileDescription "VerdantGolem"、OriginalFilename "verdantgolem.exe"；Docker WORKDIR/ENTRYPOINT `/bin/verdantgolem`、compose service 名与容器内目录；egg 文件 `egg-verdantgolem.json` 及其 startup `./verdantgolem`；rust.yml artifact `verdantgolem-{arch}-{os}` 与 `dist/verdantgolem*`；flake `--package verdantgolem` 与 importTOML 新路径；typos.toml 排除 `crates/verdantgolem-data/src/generated` 并收录 verdantgolem 词；docker.yml 仓库守卫 `VerdantGolemMC/VerdantGolem` 与 `VERDANTGOLEM_VERSION`。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A30 | passed | specs/project-identity/spec.md | **运行时显示**：server brand `"VerdantGolem"`（minecraft:brand plugin message）；默认 MOTD 含 VerdantGolem；启动 banner；crash 报告头/标签；默认 level_name "VerdantGolem world"、server_version "VerdantGolem Rust Server"；插件加载器错误文案中的品牌词。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A31 | passed | specs/project-identity/spec.md | **命令**：`/verdantgolem`（别名 version、ver），源文件 `verdantgolem.rs`，注册键与 dispatcher 测试同步；命令描述翻译值使用 VerdantGolem；GitHub API 与 issue 链接指向 `https://github.com/VerdantGolemMC/VerdantGolem`。翻译键名 `commands.pumpkin.*` 与查询串 `pumpkin:commands.pumpkin.*` 保留（属于 pumpkin: 翻译命名空间契约），仅修改 JSON 值中的品牌词与 GitHub 链接。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮抽样确认契约与内部标识符保留） |
| A32 | passed | specs/project-identity/spec.md | **文档**：README 标题与正文以 VerdantGolem 为主体，声明为 Pumpkin 的生电特性分支（carpet 式还原生电特性的目标）；CONTRIBUTING、devcontainer 名称、doc 注释中指本项目的品牌词、截图文件名 verdantgolem-chunk-loading.webp；上游文档/社区/捐赠链接作为出处保留。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A33 | passed | specs/project-identity/spec.md | **代码生成**：codegen `OUT_DIR` → `crates/verdantgolem-data/src/generated`、`MAPPING_OUT_DIR` → 主 crate 新路径；generated 文件与 WIT mapping 重新生成，内嵌 crate 路径为 `verdantgolem_util/verdantgolem_codecs` 等。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A34 | passed | specs/project-identity/spec.md | 一切原版南瓜标识：`minecraft:pumpkin*` 命名空间串、`PUMPKIN/PUMPKIN_STEM/CARVED_PUMPKIN/JACK_O_LANTERN/PUMPKIN_PIE/PUMPKIN_SEEDS` 枚举与常量、`PumpkinBlock/CarvedPumpkinBlock` 游戏 struct、`patch_pumpkin` 等 worldgen、recipe/loot datapack 文件与内容、`BlockPumpkinCarve` 音效、`block.pumpkin.*`/`tile.pumpkin.name` 翻译键、`assets/` 下原版数据文件。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮抽样确认契约与内部标识符保留） |
| A35 | passed | specs/project-identity/spec.md | NBT 键 `"PumpkinCustomData"`（实体/区块/世界自定义数据）。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A36 | passed | specs/project-identity/spec.md | 世界目录文件 `pumpkin_custom_data.nbt`。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A37 | passed | specs/project-identity/spec.md | 配置文件名 `pumpkin.toml`（及 .gitignore/.dockerignore 对应行）。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A38 | passed | specs/project-identity/spec.md | 标识符命名空间值 `"pumpkin"`：`PUMPKIN_NAMESPACE`、recipe_id `pumpkin:recipe_{n}`、Bedrock 皮肤 id `pumpkin:{uuid}`、物品自定义数据键 `pumpkin:enchantments`、翻译注册 `pumpkin:{key}`、命令树命名空间列表与 `pumpkin:command.*` 注册键的命名空间段。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮抽样确认契约与内部标识符保留） |
| A39 | passed | specs/project-identity/spec.md | WIT 包名 `pumpkin:plugin@0.1.0` 及全部生成/手写代码中的 `pumpkin::plugin` 模块路径。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮抽样确认契约与内部标识符保留） |
| A40 | passed | specs/project-identity/spec.md | 原生插件 ABI 符号 `PUMPKIN_API_VERSION`（api-macros 生成端与 native 加载端）。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A41 | passed | specs/project-identity/spec.md | wasm custom section `pumpkin.metadata`/`pumpkin.signature`、市场公钥与 `market.pumpkinmc.org` 全部 URL、HTTP `User-Agent: "Pumpkin-MC"`。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮抽样确认契约与内部标识符保留） |
| A42 | passed | specs/project-identity/spec.md | 子模块 `crates/pumpkin-plugin-wit`（见 1.2）。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A43 | passed | specs/project-identity/spec.md | 类型名（`PumpkinServer/PumpkinConfig/PumpkinError/PumpkinCommandCompleter/PumpkinMetadata` 等）、宏名（`pumpkin_block/pumpkin_block_from_tag`）、函数/访问器名（`get_pumpkin_block/get_pumpkin_version`）、常量与静态名（`PUMPKIN_API_VERSION/PUMPKIN_METADATA_SECTION/PUMPKIN_EN_US_JSON` 等含既有拼写）、日志 target `"pumpkin_plugin"`。这些符号的名称不因本 change 改变；其所在 crate 路径改变。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（本轮抽样确认契约与内部标识符保留） |
| A44 | passed | specs/project-identity/spec.md | 根 LICENSE（GPLv3）与 plugin-api 的 MIT/Apache 双许可文件内容不变。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A45 | passed | specs/project-identity/spec.md | NOTICE.md 保留对 Pumpkin 源码与 Plugin API 的出处表述，仅更新其中涉及的 crate 目录路径。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A46 | passed | specs/project-identity/spec.md | README 明确 VerdantGolem 是 Pumpkin-MC/Pumpkin 的 fork，生电特性分支，保留指向上游的文档/社区/捐赠链接。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A47 | passed | specs/project-identity/spec.md | 游戏逻辑、协议行为、功能语义零变化；仅品牌字符串与标识符路径变化。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A48 | passed | specs/project-identity/spec.md | 旧 Pumpkin 世界/配置无需迁移即可继续使用（键名未变）。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |
| A49 | passed | specs/project-identity/spec.md | 替换必须区分大小写并限定品牌语境模式（`pumpkin_`/`pumpkin::`/`"pumpkin:`/crate 名/指项目的 `Pumpkin` 词），游戏内容 token 零触碰；`translate_cross` 宏的路径段匹配表与调用点同步为 `verdantgolem_data`。 | 第 1 轮证据 + 本轮 f490fec76 仅注释级修复未影响该项（4 行全部为注释，无代码语义变化） |

## Checks

_No Runtime checks were recorded._

## Blockers

- **user**: The generic Skill bridge cannot prove an independent Verifier execution; user confirmation is required before Archive. — next: `await-user`

## Risks and skipped work

- 本轮为只读复核，未重跑 cargo（4 行纯注释改动且父提交 CI 全绿；GitHub CI 将随下次推送覆盖）
- docs/comet 映射表中的旧名属变更自述文档，非品牌残留

## Previous iterations

| Goal cycle | Iteration | Attempt | Outcome | Unresolved | Summary | Completed |
| ---: | ---: | ---: | --- | --- | --- | --- |
| 1 | 1 | 1 | fail | A4 | 49 项中 48 项通过；唯一失败项 A4 存在 3 处注释级品牌残留（旧 crate 名引用与两处项目自称），修复为小提交即可整体通过。其余：CI 全绿、生成文件零语义差异、保留契约零回归。 | 2026-08-22T11:31:30.058Z |
| 1 | 2 | 1 | pass | — | 第 2 轮复核通过（49/49）。f490fec76 与声称一致：3 文件 4 行注释级改动，A4 三处残留清除，全仓抽样无新问题，其余 48 项维持第 1 轮通过结论。verdict: pass。 | 2026-08-22T11:46:34.867Z |

## Conclusion

第 2 轮复核通过（49/49）。f490fec76 与声称一致：3 文件 4 行注释级改动，A4 三处残留清除，全仓抽样无新问题，其余 48 项维持第 1 轮通过结论。verdict: pass。
