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
| `tntDoNotUpdate` | `false` | TNT 不被方块更新引爆（可贴电源放置） |
| `tntRandomRange` | `-1` | 固定 TNT 爆炸半径（-1 = 原版随机） |
| `hardcodeTNTangle` | `-1` | 固定 TNT 抛射水平角（-1 = 原版随机） |
| `mergeTNT` | `false` | 静止的已点燃 TNT 合并为一个实体（保留最短引信） |

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
| `forceloadLimit` | `256` | /forceload 区块数上限 |
| `spawnChunkRadius` | `0` | 出生点常驻区块半径（无需 /forceload） |
| `movableAmethyst` | `false` | 活塞可推动紫水晶块/紫水晶母岩 |
| `renewableDeepslate` | `false` | y<0 岩浆遇水生成深板岩而非圆石 |
| `renewableBlackstone` | `false` | 岩浆流过蓝冰（无灵魂沙）生成黑石 |
| `desertShrubs` | `false` | 干热群系的树苗枯死为枯灌木（4 格内有水除外） |
| `renewableSponges` | `false` | 雷击守卫者转化为远古守卫者 |
| `rotatorBlock` | `false` | 发射器内的仙人掌逆时针旋转面前的方块（不消耗） |

## 命令

| 命令 | 权限 | 说明 |
| --- | --- | --- |
| `/carpet` / `/vgcarpet` | OP2 | 规则查询/设置/列表/恢复默认 |
| `/counter [颜色] [reset]` | OP2 | 16 通道漏斗计数器读取/重置 |
| `/spawn mobcaps` | OP2 | 全类别实时生物上限 |
| `/spawn tracking` | OP2 | 自然刷怪采样统计（再执行一次停止并输出报告） |
| `/perimeterinfo [pos]` | OP2 | 周边 33×33 柱的怪物可刷点扫描（含分层统计） |
| `/player <名> spawn\|kill\|attack\|sneak\|jump\|drop\|mount\|dismount\|look\|stop` | OP2 | 假人全套控制 |
| `/player list` | OP2 | 在线假人清单 |
| `/draw sphere\|ball <中心> <半径> <方块>` | OP2 | 几何绘制（只替换空气，遵守 fillUpdates） |
| `/info <坐标>` | OP2 | 方块/状态/方块实体 NBT 转储 |
| `/distance <from> <to>` | OP2 | 曼哈顿 + 欧氏测距 |
| `/log tps` / `/log mobcaps` / `/log stop` | 所有玩家 | actionbar 实时 TPS/MSPT 与生物上限订阅 |

## 假人（fake players）

- 使用与原版一致的离线 UUID（同名跨重启数据稳定），生成于命令发送者的位置与朝向
- 对所有真实玩家可见（tab 列表/实体/装备/皮肤），加载区块、计入世界
- `attack` 持续以原版剑攻速节奏攻击面前锥形范围内的最近生物（自动避开玩家）
- `kill` 走完整退出流程（玩家数据保存）

## 其他生电基础

- 原版对齐的怪物 despawn（类别距离外立即消失 / 32 格外每秒 2.5% 概率 / 命名与拴绳豁免）
- 服务器根目录 `carpet_rules.json` 持久化，重启自动恢复

## 尚未实现（规划中）

假人 `use`/`move` 连续动作、`/spawn tracking` 刷怪采样、`stackableShulkerBoxes`、
`movableBlockEntities`、QC 准连接相关规则。
