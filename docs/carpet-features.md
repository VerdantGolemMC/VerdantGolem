# VerdantGolem 生电特性（carpet 式）

VerdantGolem 内置与 [fabric-carpet](https://github.com/gnembon/fabric-carpet) 对齐的生电特性集：
规则（rules）、漏斗计数器、假人、日志订阅与一整套技术向命令。规则通过 `/carpet`
在运行时修改，按名称持久化到服务器根目录的 `carpet_rules.json`（未知/损坏条目自动回落默认值）。

## 规则（27 条）

用 `/carpet list [类别]` 浏览，`/carpet <规则> [值]` 查询/设置，`/carpet default <规则>` 恢复默认。

### tnt（爆炸与 TNT 机械）
| 规则 | 默认 | 说明 |
| --- | --- | --- |
| `explosionNoBlockDamage` | `false` | 爆炸不破坏方块 |
| `explosionNoEntityDamage` | `false` | 爆炸不伤害实体 |
| `tntPrimerMomentumRemoved` | `false` | 点燃的 TNT 无随机水平动量 |
| `tntDoNotUpdate` | `false` | 放置 TNT 时忽略已有供电；后续邻居更新仍可引爆 |
| `tntRandomRange` | `-1` | 固定 TNT 爆炸射线的随机系数（-1 = 原版随机），不替代爆炸强度 |
| `hardcodeTNTangle` | `-1` | 固定 TNT 抛射水平角（-1 = 原版随机） |
| `mergeTNT` | `false` | 仅合并位置、速度、落地状态、引信与强度相同的静止 TNT，保留爆炸次数 |

### creative（建造与参数）
| 规则 | 默认 | 说明 |
| --- | --- | --- |
| `fillUpdates` | `true` | /fill /clone /setblock /draw 是否触发方块更新 |
| `fillLimit` | `32768` | /fill 与 /clone 的体积上限 |
| `pushLimit` | `12` | 活塞推动方块数上限 |
| `railPowerLimit` | `9` | 充能铁轨能量传播距离 |
| `pingPlayerListLimit` | `12` | ping 状态响应中的玩家样本上限 |
| `creativePlayersLoadChunks` | `true` | 创造玩家是否加载区块（false=按旁观者处理） |

### survival（生存）
| 规则 | 默认 | 说明 |
| --- | --- | --- |
| `xpNoCooldown` | `false` | 玩家无延迟吸收经验球 |
| `missingTools` | `false` | 镐以镐速破坏玻璃 |

### optimization（性能）
| 规则 | 默认 | 说明 |
| --- | --- | --- |
| `maxEntityCollisions` | `0` | 单实体每 tick 碰撞处理上限（0=不限） |
| `momentumClampThreshold` | `0.003` | 动量归零阈值（0=关闭原版钳制） |
| `mobCapMultiplier` | `1.0` | 全局生物上限倍率 |

### feature（可再生与特性）
| 规则 | 默认 | 说明 |
| --- | --- | --- |
| `hopperCounters` | `false` | 漏斗向羊毛转运即计数销毁（16 通道，配 /counter） |
| `forceloadLimit` | `256` | /forceload 单次区域操作的区块数上限 |
| `spawnChunkRadius` | `0` | 主世界出生点常驻区块半径（0 使用默认半径 2，无需 /forceload） |
| `movableAmethyst` | `false` | 活塞可推动紫水晶块/紫水晶母岩 |
| `renewableDeepslate` | `false` | y<0 岩浆遇水生成深板岩而非圆石 |
| `renewableBlackstone` | `false` | 岩浆流过蓝冰（无灵魂沙）生成黑石 |
| `desertShrubs` | `false` | 沙漠群系的树苗在附近缺水时枯死为枯灌木 |
| `renewableSponges` | `false` | 雷击守卫者转化为远古守卫者 |
| `rotatorBlock` | `false` | 发射器内的仙人掌顺时针旋转面前的方块；目标状态不变时也不消耗或弹出仙人掌 |

## 命令

| 命令 | 权限 | 说明 |
| --- | --- | --- |
| `/carpet` / `/vgcarpet` | OP2 | 规则查询/设置/列表/恢复默认 |
| `/counter [颜色] [realtime\|reset]` | OP2 | 16 通道累计计数、每小时产率、计时与重置 |
| `/spawn mobcaps` | OP2 | 全类别实时生物上限 |
| `/spawn tracking` | OP2 | 自然刷怪采样统计（再执行一次停止并输出报告） |
| `/perimeterinfo [pos]` | OP2 | 周边 33×33 柱、中心上下 32 格内的僵尸代表性可刷点扫描（含分层统计） |
| `/player <名> spawn\|kill\|attack\|use\|move\|sneak\|jump\|drop\|mount\|dismount\|look\|stop` | OP2 | 假人生成、动作调度、交互与移动，完整语法见下文 |
| `/player list` | OP2 | 在线假人清单 |
| `/draw sphere\|ball <中心> <半径> <方块>` | OP2 | 几何绘制（只替换空气，遵守 fillUpdates） |
| `/info <坐标>` | OP2 | 方块/状态/方块实体 NBT 转储 |
| `/distance <from> <to>` | OP2 | 曼哈顿 + 欧氏测距 |
| `/log tps` / `/log mobcaps` / `/log stop` | 所有玩家 | actionbar 实时 TPS/MSPT 与生物上限订阅 |

## 漏斗测产

启用 `/carpet hopperCounters true`，让漏斗朝向对应颜色的羊毛。进入计数器的物品会被销毁，
不是储存在羊毛中；正式生产线应在测量后关闭规则或切回储物容器。

```text
/counter                       # 所有已开始计时通道的累计量、游戏时间产率与耗时
/counter white                 # 白色通道，含每种物品的数量和 items/h
/counter realtime              # 所有通道按真实经过时间统计
/counter white realtime        # 白色通道的真实时间产率
/counter white reset           # 清空白色通道，并从现在开始计时
/counter reset                 # 清空全部通道，并同时开始新的计时
```

- 默认以 **20 个实际模拟 tick = 1 秒** 折算游戏时间；多维度共用同一时钟。
  `/tick freeze` 的网络循环不计入，`/tick step` 和 sprint 执行的模拟 tick 会计入。
- `realtime` 使用真实经过时间，包含暂停和卡顿；因此修改 tick rate 或使用 sprint 时，
  两种产率会不同。默认产率适合比较农场机制，真实产率适合测量实际吞吐量。
- 未重置的新通道从第一批非零物品进入时开始计时；显式 reset 则立即开始计时。
  时间尚未推进时显示 `pending`，而不是无穷大或误导性的产率。
- 同一次漏斗转运中的物品以一批记入，读取和重置不会看到半批统计。
  计数器只保存在内存中，服务器重启后清空；这与下面的强加载清单不同。

## 强加载与重启恢复

`/forceload add`、`remove`、`remove all` 同时管理实际区块加载 ticket 和磁盘清单。
即使没有玩家在线，重启后也会恢复清单中的加载 ticket。
`/forceload query` 列出当前维度的全部清单并返回数量；`query <x> <z>` 只验证指定位置，
该区块未强加载时命令失败，因此可供 `execute store result` / `store success` 使用。

- 每个维度使用自身存档目录的 `data/minecraft/chunks.dat`，格式为 gzip NBT 的
  `data.Forced` long array，保留已有的其他 NBT 字段。
- 当上述文件不存在时，兼容读取并继续更新该维度旧位置的 `data/chunks.dat`。
  两者同时存在时使用前者，避免旧清单重新激活已移除的区块。
- 每次变更先原子写入清单，再更新内存和 ticket；写入失败会向命令发送者明确报告。
- 损坏、不可读或包含越界坐标的清单保留原文件，并阻止该维度的 `/forceload` 读写操作。
  修复文件后重新加载世界/重启服务器，不会静默以空清单覆盖存档。

## 假人（fake players）

- 使用与原版一致的离线 UUID（同名跨重启数据稳定）；由玩家生成时使用玩家的位置与朝向，
  命令方块生成时使用该世界的出生点
- 对所有真实玩家可见（tab 列表/实体/装备/皮肤），加载区块、计入世界
- `attack` 攻击视线中的实体，检查交互距离和方块遮挡，沿用普通玩家的攻击、PvP 和插件回调
- `use` 共用普通玩家的实体交互、方块交互、放置、物品使用、库存消耗及冷却逻辑；
  主手目标交互未处理时尝试主手空中使用，再考虑副手，插件取消会终止本次动作
- `move` 设置相对于当前朝向的移动输入，使用实体碰撞、重力和区块跟踪，而不是直接传送
- `kill` 走完整退出流程（玩家数据保存）

```text
/player farm_bot spawn
/player farm_bot look 0 0
/player farm_bot attack                  # 保留原有攻击开关：再执行一次停止攻击
/player farm_bot attack once
/player farm_bot attack continuous
/player farm_bot attack interval 12
/player farm_bot use                     # 默认等同 use once
/player farm_bot use once
/player farm_bot use continuous
/player farm_bot use interval 20
/player farm_bot move                    # 默认向前持续移动
/player farm_bot move forward
/player farm_bot move backward
/player farm_bot move left
/player farm_bot move right
/player farm_bot move stop               # 只停止移动输入，保留攻击/使用调度
/player farm_bot stop                    # 清除动作、移动、潜行/疾跑，并松开持用物品
/player farm_bot kill
```

`once` 在下一个实际游戏 tick 执行一次；`interval N` 首次也在下一游戏 tick 执行，
随后严格间隔 N tick（N 为 1 到 4294967295 的整数）。攻击和使用的调度彼此独立。
连续攻击每个游戏 tick 尝试一次，实际伤害仍经过普通玩家的武器冷却计算；
需要低频攻击时使用例如 `attack interval 12`，不会额外叠加旧的固定延迟。

连续 `use` 对瞬时使用每 4 tick 尝试一次，食物、弓等持用动作保持按住，而不是不断重置。
`once` 和 `interval` 的单次按键在下一 tick 松开；食用和蓄力应使用 `continuous`。
移动停止后保留正常摩擦惯性；骑乘时不额外驱动假人自身步行。

冻结服务器也会冻结假人动作延迟、物品使用时长与物理；step/sprint 随真实模拟 tick 推进。
死亡、旁观和睡眠会暂停调度并清除持用/移动输入。同名玩家数据可跨重启读取，
在线假人清单和动作调度本身不自动跨重启恢复。

## 其他生电基础

- 原版对齐的怪物 despawn（类别距离外立即消失 / 32 格外每秒 2.5% 概率 / 命名与拴绳豁免）
- 服务器根目录 `carpet_rules.json` 持久化，重启自动恢复

## 尚未实现（规划中）

`stackableShulkerBoxes`、`movableBlockEntities`、QC 准连接相关规则，以及假人的方块采掘动作。
`/spawn tracking` 已接入成功自然刷怪的实际路径，按维度与实体种类报告统计。

## 验证

开发与独立临时世界验证见 [生电回归测试](technical-survival-testing.md)。
