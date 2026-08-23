# VerdantGolem Carpet（verdantgolem）审查交接记录

> 交接对象：下一个 Codex/开发 agent  
> 记录日期：2026-08-23（Asia/Shanghai）  
> 仓库：VerdantGolemMC/VerdantGolem  
> 工作目录：/Users/mcxiaocai666/Downloads/VerdantGolem

## 0. 结论先行

当前工作目标是：从头到尾审查项目内所有 Carpet（verdantgolem）实现，修复 bug，完成本地验证，提交并推送 master，最后由 GitHub Actions 生成 Nightly Release。

当前没有提交、没有推送、没有发布 Release。修复全部留在本地分支 codex/carpet-full-audit-fixes 的未提交工作树中。

本地工作树已经包含三名专项 agent 的修复，以及主 agent 最后一轮交叉审查补丁；但最后一轮改动尚未完成全量编译和测试，当前不能当作可发布状态。

接手后的第一优先级：

1. 运行 cargo fmt --all，因为最后一轮改动后 format check 目前会报告差异。
2. 运行 cargo check -p verdantgolem，先解决编译错误。
3. 重跑针对测试，再运行 workspace 测试、Clippy 和 release 构建。
4. 审查 diff，提交修复分支，快进/合并到本地 master，推送 origin master。
5. 等待 .github/workflows/rust.yml，确认 nightly tag 和 Nightly Release 产物。

## 1. Git、分支和远端

### 当前状态

~~~text
branch: codex/carpet-full-audit-fixes
base/HEAD before uncommitted work: 97c5da284
master: 97c5da284 (origin/master)
origin: https://github.com/VerdantGolemMC/VerdantGolem.git
default branch: master
GitHub viewer permission: ADMIN
~~~

当前修复 diff 约为 35 个已修改文件、2200 行新增、494 行删除；格式化后数字会变化。

当前已修改文件范围包括：

~~~text
Cargo.lock
crates/verdantgolem-data/src/blocks.rs
crates/verdantgolem/Cargo.toml
crates/verdantgolem/src/block/blocks/piston/mod.rs
crates/verdantgolem/src/block/blocks/plant/sapling.rs
crates/verdantgolem/src/block/blocks/redstone/rails/powered_rail.rs
crates/verdantgolem/src/block/blocks/tnt.rs
crates/verdantgolem/src/block/entities/hopper.rs
crates/verdantgolem/src/block/fluid/lava.rs
crates/verdantgolem/src/carpet/counters.rs
crates/verdantgolem/src/carpet/fake_player.rs
crates/verdantgolem/src/carpet/loggers.rs
crates/verdantgolem/src/carpet/registry.rs
crates/verdantgolem/src/carpet/spawn_tracking.rs
crates/verdantgolem/src/command/commands/carpet.rs
crates/verdantgolem/src/command/commands/clone.rs
crates/verdantgolem/src/command/commands/counter.rs
crates/verdantgolem/src/command/commands/draw.rs
crates/verdantgolem/src/command/commands/fill.rs
crates/verdantgolem/src/command/commands/forceload.rs
crates/verdantgolem/src/command/commands/perimeterinfo.rs
crates/verdantgolem/src/command/commands/player.rs
crates/verdantgolem/src/command/commands/setblock.rs
crates/verdantgolem/src/command/commands/spawn.rs
crates/verdantgolem/src/entity/experience_orb.rs
crates/verdantgolem/src/entity/living.rs
crates/verdantgolem/src/entity/mob/mod.rs
crates/verdantgolem/src/entity/mod.rs
crates/verdantgolem/src/entity/player.rs
crates/verdantgolem/src/entity/tnt.rs
crates/verdantgolem/src/server/connection_cache.rs
crates/verdantgolem/src/world/chunker.rs
crates/verdantgolem/src/world/explosion.rs
crates/verdantgolem/src/world/mod.rs
crates/verdantgolem/src/world/natural_spawner.rs
~~~

### 不要误提交的文件

以下未跟踪项属于环境/仓库上下文，不属于本次修复，不要加入提交：

~~~text
.dsh/
.zcode/
AGENTS.md
CLAUDE.md
~~~

本交接文档 CARPET_AUDIT_HANDOFF.md 是本次新增文档；是否和修复一起提交由接手 agent 决定。

### Comet

按仓库 AGENTS.md 运行过：

~~~bash
comet resume-probe . --stdin --json
~~~

结果为 workflow=native、skill=comet-native，但 action=none、reasonCode=no-active-native-changes，没有需要恢复的活跃 workflow。

## 2. 三个专项 agent 已完成的工作

三个专项 agent 均已结束，没有提交 commit，修改直接留在共享工作树。

### 2.1 core：registry、Carpet 命令、TNT、爆炸

主要文件：

- crates/verdantgolem/src/carpet/registry.rs
- crates/verdantgolem/src/command/commands/carpet.rs
- crates/verdantgolem/src/entity/tnt.rs
- crates/verdantgolem/src/world/explosion.rs

已完成：

- Rule 值类型、有限浮点、min/max 边界检查。
- tntRandomRange 保持官方语义：允许 -1 或任意有限非负值，不新增 max=1.0。
- hardcodeTNTangle 保持 -1 或 [0, 2π)；合法角度优先于 tntPrimerMomentumRemoved。
- tntDoNotUpdate 只禁止放置时立即 prime；邻居更新仍可点燃 TNT。
- 规则 JSON 持久化增加锁、原子写入、损坏回滚和错误隔离。
- JSON 整数不再错误解析成浮点规则值。
- /carpet 命令错误处理和无效值回滚。
- TNT 合并增加相同位置、速度、落地状态、fuse、power 检查；使用唯一合并者避免并发双向删除。
- TNT 合并爆炸按合并数量执行，并防止 fuse 下溢。
- tntRandomRange 只替换每条爆炸射线的随机因子，保留 TNT 基础威力和实体伤害半径。
- TNT 初速度优先级保持：合法 hardcodeTNTangle > 去随机水平动量 > vanilla 随机角度。

剩余风险：极大但合法的 tntRandomRange 可能带来性能压力；这是官方规则允许的边界，后续可考虑性能测试或文档说明。

### 2.2 commands：假人和 /player

主要文件：

- crates/verdantgolem/src/carpet/fake_player.rs
- crates/verdantgolem/src/command/commands/player.rs
- crates/verdantgolem/src/entity/player.rs
- crates/verdantgolem/src/entity/mod.rs
- crates/verdantgolem/Cargo.toml
- Cargo.lock

已完成：

- 假人名称对齐 vanilla：3–16 个 ASCII 字母/数字/下划线。
- 名称大小写不敏感 reservation，避免并发 spawn 重复。
- Spawning reservation 在异步失败时清理，成功转为 Online。
- vanilla 离线 UUID；新增 workspace 的 md5 依赖。
- 生成坐标、空间、区块和世界检查。
- 跨世界的 world/chunk 状态修复。
- 攻击增加方块遮挡射线检查。
- mount/dismount 增加容量、冷却、关系和事件取消检查。
- kill 同时处理玩家列表、实体/区块状态、NBT 与 advancement 保存。
- Local fake player 支持物理跳跃、移动同步、位置/旋转更新、无客户端 ACK 下马。
- /player 错误处理和测试。

专项 agent 报告通过：

~~~text
carpet::fake_player：4/4
command::commands::player::tests：1/1
~~~

### 2.3 integrations：tracking、logger、hopper、区域命令

已完成：

- /spawn tracking 使用单锁 session，按维度隔离，使用单调时间，只有成功插入世界的自然刷怪才记录，每维 top 10 后报告省略数量。
- /log tps 和 /log mobcaps 聚合为一个 action bar，使用实时 tickrate，冻结且未 step/sprint 时 TPS=0，mob cap 使用完整公式，Bedrock 使用 title/actionbar。
- hopper counter 一次清空全部槽位；加法、总计和命令返回值避免溢出；通道限制在 0..16。
- /clone 区域长度、体积和坐标使用 checked i64/u64，接入 fillUpdates。
- /draw 做 build limit、加载区块、fillLimit、vanilla 修改上限和全部目标预检，统计实际写入数量。
- /perimeterinfo 做完整高度扫描、predicate 和覆盖区块校验。
- /forceload checked 面积和命令结果饱和。

专项 agent 报告的针对性测试共 19 项，0 失败：

~~~text
spawn tracking 3
counters 4
loggers 3
mobcap 2
forceload math 2
/counter 2
/draw 1
/clone 2
~~~

## 3. 主 agent 最后一轮刚完成的交叉修复

以下改动发生在专项测试之后，必须重新编译验证。

### 3.1 通用方块旋转

文件：crates/verdantgolem-data/src/blocks.rs

Block::rotate 从返回原状态改为重建状态并旋转：

- facing 值；
- axis 值；
- 数字 rotation；
- rail shape（直轨、上坡轨、弯轨）；
- jigsaw/crafter orientation；
- fence、wall、pane、redstone/multiface 等将方向编码在属性键名中的 north/east/south/west。

新增了 directional/axis、连接属性键、orientation 测试。

必须确认：

- Block::OAK_FENCE、Block::JIGSAW 及 from_properties API 可用；
- 属性键重排后状态 ID 正确；
- 属性向量顺序不会破坏 from_properties；
- 所有带方向键的 block 都确实应该做方向置换。

### 3.2 普通 Mob 的实体推挤

文件：crates/verdantgolem/src/entity/mob/mod.rs

交叉审查发现 blanket impl<T: Mob> EntityBase for T 没有转发 is_pushable，所以新增的 maxEntityCollisions 路径对普通 mob 实际仍把它们视为不可推动。

已补：

~~~rust
fn is_pushable(&self) -> bool {
    self.get_mob_entity().living_entity.is_pushable()
}
~~~

需要运行 compile/test，并最好补 mob↔mob 或 mob↔player 推挤验证。

### 3.3 /forceload 真实 chunk ticket

文件：crates/verdantgolem/src/command/commands/forceload.rs

交叉审查发现旧实现只写 World::forced_chunks，未调用现成的 ChunkLoading::add_force_ticket / remove_force_ticket。

当前补丁：

- add 仅对实际新增坐标调用 add_force_ticket；
- remove 仅对实际移除坐标调用 remove_force_ticket；
- remove all 对所有现有坐标移除 ticket；
- 返回实际变化数量而不是请求区域数量；
- 空 add/remove 返回失败；
- 保留 checked 面积和 forceloadLimit。

必须验证：

- forced_chunks 与 ChunkLoading.ticket 在世界重载/初始化时一致；
- update_active_chunks 不会重复或冲突；
- query 是否还应校验 ticket；
- drain、锁和 ticket remove 的编译及运行语义。

### 3.4 fillUpdates 和区域体积

文件：

- crates/verdantgolem/src/command/commands/fill.rs
- crates/verdantgolem/src/command/commands/setblock.rs
- crates/verdantgolem/src/command/commands/clone.rs
- crates/verdantgolem/src/command/commands/draw.rs

当前补丁：

- /fill 使用 fill_flags；fillUpdates=false 时使用 FORCE_STATE | SKIP_BLOCK_ADDED_CALLBACK，并跳过额外邻居更新；
- /setblock 同样跳过 added callback；
- /clone、/draw 已有对应 flags；
- /fill 体积使用 checked i64/u64，避免坐标极值减法/乘法溢出；
- 增加 volume/flags 单元测试。

必须验证：

- Strict 模式是否仍应独立于 fillUpdates；
- fillLimit 和 vanilla max_block_modifications 的顺序/错误信息；
- destroy 模式的 break/set flags 是否保留正确掉落和替换副作用；
- 官方 impendingFillSkipUpdates 的完整 side effect parity 是否还需扩展。

## 4. Carpet 规则和接线概览

规则目录当前包含：

~~~text
explosionNoBlockDamage
explosionNoEntityDamage
tntPrimerMomentumRemoved
tntDoNotUpdate
tntRandomRange
hardcodeTNTangle
mergeTNT
hopperCounters
renewableSponges
movableAmethyst
renewableDeepslate
renewableBlackstone
missingTools
desertShrubs
rotatorBlock
fillUpdates
fillLimit
maxEntityCollisions
momentumClampThreshold
xpNoCooldown
mobCapMultiplier
forceloadLimit
spawnChunkRadius
creativePlayersLoadChunks
pushLimit
railPowerLimit
pingPlayerListLimit
~~~

已接线的行为包括：

- 爆炸方块/实体伤害；
- TNT 动量、放置更新、随机射线、角度、合并；
- hopper counters；
- renewable sponges；
- movable amethyst；
- Overworld、y<0 的 renewable deepslate；
- 排除 DOWN 蓝冰的 renewable blackstone；
- desert pyramid biome tag、流体水和 Y-4..Y+1 检查；
- missingTools 玻璃挖掘；
- rotatorBlock；
- fill/clone/setblock/draw 更新；
- fillLimit；
- maxEntityCollisions，0=无限；
- momentumClampThreshold，0=关闭；
- xpNoCooldown；
- mobCapMultiplier；
- forceloadLimit；
- spawnChunkRadius；
- creativePlayersLoadChunks；
- pushLimit；
- railPowerLimit；
- pingPlayerListLimit。

需要继续检查是否存在运行时规则读取快照、重复 ticket 或遗漏硬编码上限。

## 5. 已运行与未运行的验证

### 已报告通过

专项 agent 报告：

~~~text
spawn tracking：3
counters：4
loggers：3
mob cap：2
forceload math：2
counter command：2
draw：1
clone：2
合计：19 项，0 失败

fake_player：4/4
player command：1/1
~~~

这些测试是在最后新增方块旋转、Mob push、真实 forceload ticket、fill/setblock flags 之前完成的，必须重跑。

### 当前工作树现场检查

~~~text
没有 cargo/rustc 进程在后台运行。
git diff --check：通过，没有空白错误。
cargo fmt --all -- --check：当前失败，报告尚未格式化的差异。
~~~

最后一次 format check 报告的文件：

- crates/verdantgolem/src/block/blocks/redstone/rails/powered_rail.rs
- crates/verdantgolem/src/command/commands/fill.rs
- crates/verdantgolem/src/command/commands/setblock.rs
- crates/verdantgolem/src/world/mod.rs
- crates/verdantgolem-data/src/blocks.rs

建议先运行：

~~~bash
cargo fmt --all
git diff --check
~~~

### 最终本轮尚未完成

~~~bash
cargo check -p verdantgolem
cargo test -p verdantgolem-data blocks::tests --lib
cargo test -p verdantgolem carpet:: --lib
cargo test -p verdantgolem --lib
cargo test --workspace
cargo clippy --all-targets --all-features
cargo clippy --release --all-targets --all-features
cargo build --release
~~~

较早阶段曾有 cargo check -p verdantgolem --lib 通过，但发生在本轮最后修改前，不能替代当前工作树验证。

## 6. 接手执行顺序

### 第一步：格式化并保存现场

~~~bash
cd /Users/mcxiaocai666/Downloads/VerdantGolem
git status --short --branch
git diff --check
cargo fmt --all
git diff --check
~~~

不要运行 git reset --hard 或 git checkout --，当前未提交 diff 都是修复现场。

### 第二步：快速编译

~~~bash
cargo check -p verdantgolem-data --lib
cargo check -p verdantgolem --lib
~~~

优先排查：

- World::spawn_entity 从 () 改为 bool 后的所有调用点；
- Block::rotate 的属性重建；
- ChunkLoading ticket 锁和 remove 配对；
- Mob blanket EntityBase 的 is_pushable；
- fill.rs 的 Context.block_flags 和 checked volume。

辅助搜索：

~~~bash
rg -n "spawn_entity\\(" crates/verdantgolem/src -g '*.rs'
rg -n "setblock_flags|fill_flags|checked_region_volume|rotate_orientation|add_force_ticket|remove_force_ticket" crates/verdantgolem/src crates/verdantgolem-data/src -g '*.rs'
~~~

### 第三步：针对测试

~~~bash
cargo test -p verdantgolem-data blocks::tests --lib
cargo test -p verdantgolem carpet::registry --lib
cargo test -p verdantgolem carpet::spawn_tracking --lib
cargo test -p verdantgolem carpet::loggers --lib
cargo test -p verdantgolem carpet::counters --lib
cargo test -p verdantgolem carpet::fake_player --lib
cargo test -p verdantgolem command::commands::clone::tests --lib
cargo test -p verdantgolem command::commands::counter::tests --lib
cargo test -p verdantgolem command::commands::draw::tests --lib
cargo test -p verdantgolem command::commands::fill::tests --lib
cargo test -p verdantgolem command::commands::forceload::tests --lib
cargo test -p verdantgolem command::commands::player::tests --lib
cargo test -p verdantgolem command::commands::setblock::tests --lib
cargo test -p verdantgolem world::natural_spawner::tests --lib
cargo test -p verdantgolem world::explosion::tests --lib
cargo test -p verdantgolem entity::tnt::tests --lib
~~~

### 第四步：完整检查

~~~bash
cargo test --workspace
cargo clippy --all-targets --all-features
cargo clippy --release --all-targets --all-features
cargo build --release
~~~

CI 还会运行 format、cargo-machete、debug/release Clippy、nextest、doctest 和多平台 release build。

### 第五步：提交前检查

~~~bash
git diff --stat
git diff -- crates/verdantgolem/src/carpet/registry.rs
git diff -- crates/verdantgolem/src/entity/tnt.rs crates/verdantgolem/src/world/explosion.rs
git diff -- crates/verdantgolem/src/carpet/fake_player.rs crates/verdantgolem/src/command/commands/player.rs
git diff -- crates/verdantgolem/src/command/commands/forceload.rs
git status --short
~~~

确认不包含四个环境未跟踪项；确认没有临时文件、重复 chunk ticket、静态规则快照或 TODO 占位逻辑。

## 7. 提交和 GitHub Release 流程

完成本地验证后，不要用 git add -A：

~~~bash
git add Cargo.lock crates/verdantgolem-data/src/blocks.rs crates/verdantgolem/Cargo.toml crates/verdantgolem/src
git add CARPET_AUDIT_HANDOFF.md
git status --short
git commit -m "fix: complete Carpet audit and runtime rule wiring"
~~~

然后快进到 master 并推送：

~~~bash
git switch master
git merge --ff-only codex/carpet-full-audit-fixes
git push origin master
~~~

若远端 master 在期间发生变化，先停下重新审查分支关系，不要强推覆盖其他提交。

.github/workflows/rust.yml 的发布行为：

- push master 触发完整 CI；
- 构建 Linux、ARM Linux、macOS；
- 仅 refs/heads/master 进入 draft_release；
- 强制更新 nightly tag；
- 创建/更新 prerelease Nightly Build；
- 附件命名为 verdantgolem-*。

推送后：

~~~bash
gh run list --repo VerdantGolemMC/VerdantGolem --limit 10
gh run watch --repo VerdantGolemMC/VerdantGolem <RUN_ID>
gh release view nightly --repo VerdantGolemMC/VerdantGolem
gh release download nightly --repo VerdantGolemMC/VerdantGolem --dir /tmp/verdantgolem-nightly-check
~~~

最终回复要记录 commit SHA、Actions run URL、Nightly Release URL 和产物名称。

## 8. 剩余风险（按优先级）

1. P0：最终工作树尚未编译。最后一轮新代码可能有 Rust 类型、trait、借用或生成 block property API 错误。
2. P0：format check 当前失败。先运行 cargo fmt --all。
3. P1：方块旋转属性重建需要 data crate 测试，尤其是连接属性键和 jigsaw orientation。
4. P1：forceload ticket 与持久化/世界重载一致性未做集成测试。
5. P1：maxEntityCollisions 的实际 mob 推挤尚未做游戏级测试。
6. P1：fillUpdates=false 的完整 vanilla side effect parity 未做集成测试。
7. P1：spawnChunkRadius=0 当前约定为 vanilla radius 2，需要确认 ticket level 和项目 active chunk 语义。
8. P2：自然刷怪器已有 TODO/精度限制，perimeterinfo 报告精度受其影响。
9. P2：极大合法 tntRandomRange 可能增加爆炸射线工作量。
10. P2：Windows 原子替换/重命名语义要靠 CI 验证。

## 9. 交接完成标准

只有满足以下条件，才能向用户报告“全部修复并已发布”：

- 当前代码通过 cargo fmt --check；
- cargo test --workspace 成功；
- debug/release Clippy 成功；
- cargo build --release 成功；
- 修复 commit 已合并到 master 并推送；
- GitHub Actions format、machete、两个 Clippy、nextest/doctest、release build 全部成功；
- nightly Release 已生成，附件至少有 Linux、ARM Linux、macOS；
- 最终回复记录 commit、Actions 和 Release URL。

在此之前不要清理未提交修复或删除本交接文档。

