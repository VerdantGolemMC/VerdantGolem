# 项目标识（project-identity）

本 capability 定义 VerdantGolem 仓库（Pumpkin-MC/Pumpkin 的生电特性 fork）改名后的完整命名体系：哪些标识属于 VerdantGolem 项目自描述层（改名），哪些属于上游契约、游戏内容或同步友好保留层（不改名）。

## 1. 命名方案

品牌名：`VerdantGolem`（显示）、`verdantgolem`（代码/包名，一体式小写，无连字符无缩写）、`VERDANTGOLEM`（如需大写环境变量）。

### 1.1 Workspace 成员改名映射

| 旧 | 新 |
| --- | --- |
| crates/pumpkin | crates/verdantgolem |
| crates/pumpkin-api-macros | crates/verdantgolem-api-macros |
| crates/pumpkin-codecs | crates/verdantgolem-codecs |
| crates/pumpkin-config | crates/verdantgolem-config |
| crates/pumpkin-data | crates/verdantgolem-data |
| crates/pumpkin-inventory | crates/verdantgolem-inventory |
| crates/pumpkin-macros | crates/verdantgolem-macros |
| crates/pumpkin-nbt | crates/verdantgolem-nbt |
| crates/pumpkin-plugin-api | crates/verdantgolem-plugin-api |
| crates/pumpkin-plugin-utils | crates/verdantgolem-plugin-utils |
| crates/pumpkin-protocol | crates/verdantgolem-protocol |
| crates/pumpkin-util | crates/verdantgolem-util |
| crates/pumpkin-world | crates/verdantgolem-world |
| tools/pumpkin-codegen | tools/verdantgolem-codegen |
| tools/pumpkin-fuzzer | tools/verdantgolem-fuzzer |

package name、依赖键、`[workspace.dependencies]` path、`default-run`、fuzz 子 crate（verdantgolem-nbt-fuzz、verdantgolem-protocol-fuzz）与各自 Cargo.lock 条目同步。导入路径统一变为 `use verdantgolem*::`（连字符包名在 use 中即 `verdantgolem_util` 形式）。

### 1.2 例外：不随 workspace 改名

- `crates/pumpkin-plugin-wit`：git 子模块，路径与 URL（Pumpkin-MC/pumpkin-plugin-wit）不变，`.gitmodules` 及所有相对路径引用（bindgen!/wit_bindgen!、cargo-component metadata target、codegen WIT_OUT_DIR）不变。

## 2. 改名层（VerdantGolem 项目自描述）

1. **构建产物**：二进制名 `verdantgolem`；Windows 资源 FileDescription "VerdantGolem"、OriginalFilename "verdantgolem.exe"；Docker WORKDIR/ENTRYPOINT `/bin/verdantgolem`、compose service 名与容器内目录；egg 文件 `egg-verdantgolem.json` 及其 startup `./verdantgolem`；rust.yml artifact `verdantgolem-{arch}-{os}` 与 `dist/verdantgolem*`；flake `--package verdantgolem` 与 importTOML 新路径；typos.toml 排除 `crates/verdantgolem-data/src/generated` 并收录 verdantgolem 词；docker.yml 仓库守卫 `VerdantGolemMC/VerdantGolem` 与 `VERDANTGOLEM_VERSION`。
2. **运行时显示**：server brand `"VerdantGolem"`（minecraft:brand plugin message）；默认 MOTD 含 VerdantGolem；启动 banner；crash 报告头/标签；默认 level_name "VerdantGolem world"、server_version "VerdantGolem Rust Server"；插件加载器错误文案中的品牌词。
3. **命令**：`/verdantgolem`（别名 version、ver），源文件 `verdantgolem.rs`，注册键与 dispatcher 测试同步；命令描述翻译值使用 VerdantGolem；GitHub API 与 issue 链接指向 `https://github.com/VerdantGolemMC/VerdantGolem`。翻译键名 `commands.pumpkin.*` 与查询串 `pumpkin:commands.pumpkin.*` 保留（属于 pumpkin: 翻译命名空间契约），仅修改 JSON 值中的品牌词与 GitHub 链接。
4. **文档**：README 标题与正文以 VerdantGolem 为主体，声明为 Pumpkin 的生电特性分支（carpet 式还原生电特性的目标）；CONTRIBUTING、devcontainer 名称、doc 注释中指本项目的品牌词、截图文件名 verdantgolem-chunk-loading.webp；上游文档/社区/捐赠链接作为出处保留。
5. **代码生成**：codegen `OUT_DIR` → `crates/verdantgolem-data/src/generated`、`MAPPING_OUT_DIR` → 主 crate 新路径；generated 文件与 WIT mapping 重新生成，内嵌 crate 路径为 `verdantgolem_util/verdantgolem_codecs` 等。

## 3. 保留层（不因品牌改名而变化）

### 3.1 Minecraft 游戏内容

一切原版南瓜标识：`minecraft:pumpkin*` 命名空间串、`PUMPKIN/PUMPKIN_STEM/CARVED_PUMPKIN/JACK_O_LANTERN/PUMPKIN_PIE/PUMPKIN_SEEDS` 枚举与常量、`PumpkinBlock/CarvedPumpkinBlock` 游戏 struct、`patch_pumpkin` 等 worldgen、recipe/loot datapack 文件与内容、`BlockPumpkinCarve` 音效、`block.pumpkin.*`/`tile.pumpkin.name` 翻译键、`assets/` 下原版数据文件。

### 3.2 持久化与线上契约（上游 Pumpkin 对齐，用户决策：方便同步上游）

- NBT 键 `"PumpkinCustomData"`（实体/区块/世界自定义数据）。
- 世界目录文件 `pumpkin_custom_data.nbt`。
- 配置文件名 `pumpkin.toml`（及 .gitignore/.dockerignore 对应行）。
- 标识符命名空间值 `"pumpkin"`：`PUMPKIN_NAMESPACE`、recipe_id `pumpkin:recipe_{n}`、Bedrock 皮肤 id `pumpkin:{uuid}`、物品自定义数据键 `pumpkin:enchantments`、翻译注册 `pumpkin:{key}`、命令树命名空间列表与 `pumpkin:command.*` 注册键的命名空间段。

### 3.3 插件接口契约（用户决策：保持 pumpkin 生态对齐）

- WIT 包名 `pumpkin:plugin@0.1.0` 及全部生成/手写代码中的 `pumpkin::plugin` 模块路径。
- 原生插件 ABI 符号 `PUMPKIN_API_VERSION`（api-macros 生成端与 native 加载端）。
- wasm custom section `pumpkin.metadata`/`pumpkin.signature`、市场公钥与 `market.pumpkinmc.org` 全部 URL、HTTP `User-Agent: "Pumpkin-MC"`。
- 子模块 `crates/pumpkin-plugin-wit`（见 1.2）。

### 3.4 内部 Rust 标识符（同步友好）

类型名（`PumpkinServer/PumpkinConfig/PumpkinError/PumpkinCommandCompleter/PumpkinMetadata` 等）、宏名（`pumpkin_block/pumpkin_block_from_tag`）、函数/访问器名（`get_pumpkin_block/get_pumpkin_version`）、常量与静态名（`PUMPKIN_API_VERSION/PUMPKIN_METADATA_SECTION/PUMPKIN_EN_US_JSON` 等含既有拼写）、日志 target `"pumpkin_plugin"`。这些符号的名称不因本 change 改变；其所在 crate 路径改变。

## 4. 出处与许可

- 根 LICENSE（GPLv3）与 plugin-api 的 MIT/Apache 双许可文件内容不变。
- NOTICE.md 保留对 Pumpkin 源码与 Plugin API 的出处表述，仅更新其中涉及的 crate 目录路径。
- README 明确 VerdantGolem 是 Pumpkin-MC/Pumpkin 的 fork，生电特性分支，保留指向上游的文档/社区/捐赠链接。

## 5. 不变量

- 游戏逻辑、协议行为、功能语义零变化；仅品牌字符串与标识符路径变化。
- 旧 Pumpkin 世界/配置无需迁移即可继续使用（键名未变）。
- 替换必须区分大小写并限定品牌语境模式（`pumpkin_`/`pumpkin::`/`"pumpkin:`/crate 名/指项目的 `Pumpkin` 词），游戏内容 token 零触碰；`translate_cross` 宏的路径段匹配表与调用点同步为 `verdantgolem_data`。
