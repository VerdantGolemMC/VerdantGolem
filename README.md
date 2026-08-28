<div align="center">

# VerdantGolem

![CI](https://github.com/VerdantGolemMC/VerdantGolem/actions/workflows/rust.yml/badge.svg)
[![Upstream Discord](https://img.shields.io/discord/1268592337445978193.svg?label=&logo=discord&logoColor=ffffff&color=7389D8&labelColor=6A7EC2)](https://discord.gg/wT8XjrjKkf)
[![License: GPL](https://img.shields.io/badge/License-GPLv3-yellow.svg)](https://opensource.org/licenses/gpl-3-0)

</div>

[VerdantGolem](https://github.com/VerdantGolemMC/VerdantGolem) is a fork of [Pumpkin](https://pumpkinmc.org/) — a Minecraft server built
entirely in Rust — focused on faithfully restoring technical-survival (生电) mechanics, in the spirit of the
[Carpet mod](https://github.com/gnembon/fabric-carpet): vanilla-accurate redstone, mob AI, and game behavior that
technical farms and contraptions depend on.

It keeps tracking upstream Pumpkin so improvements and protocol updates can be merged continuously.
<div align="center">

![VerdantGolem Chunk Loading](./assets/verdantgolem-chunk-loading.webp)

</div>

## Goals

- **Technical survival (生电)**: Restore and preserve vanilla-accurate mechanics the way Carpet does — redstone, mob spawning, TNT, rail and light behavior that farms rely on.
- **Performance**: Leveraging multi-threading for maximum speed and efficiency.
- **Compatibility**: Supports the latest Java & Bedrock Minecraft server version while adhering to Vanilla game mechanics.
- **Security**: Prioritizes security by preventing known security exploits.
- **Flexibility**: Highly configurable, with the ability to disable unnecessary features.
- **Extensibility**: Provides a foundation for plugin development.

> [!IMPORTANT]
> VerdantGolem is currently under heavy development.
>
> [See what upstream needs before the 1.0.0 Release](https://github.com/Pumpkin-MC/Pumpkin/issues/449)

## Features

- [x] Configuration (toml)
- [Tracking: Protocol](https://github.com/Pumpkin-MC/Pumpkin/issues/1401)
  - [x] Server Status/Ping
  - [x] Encryption
  - [x] Packet Compression
  - [x] Java Edition
  - [x] Bedrock Edition (W.I.P)
  - ...
- [Tracking: World](https://github.com/Pumpkin-MC/Pumpkin/issues/1403)
  - [x] Player Tab-list
  - [x] Scoreboard
  - [x] World Loading
  - [x] World Time
  - [x] World Borders
  - [x] World Saving
  - [x] Lighting
  - [x] Entity Spawning
  - [x] Bossbar
  - [x] Chunk Loading (Vanilla, Linear, Pump)
  - [Chunk Generation](https://github.com/Pumpkin-MC/Pumpkin/issues/36)
  - [x] Chunk Saving (Vanilla, Linear, Pump)
  - [Redstone](https://github.com/Pumpkin-MC/Pumpkin/issues/1402)
  - [x] Liquid Physics
  - ...
- [Tracking: Player](https://github.com/Pumpkin-MC/Pumpkin/issues/1405)
  - [x] Skins
  - [x] Teleport
  - [x] Movement
  - [x] Animation
  - [x] Inventory
  - [Combat](https://github.com/Pumpkin-MC/Pumpkin/issues/1404)
  - [x] Experience
  - [x] Hunger
  - [X] Off Hand
  - [X] Advancements (W.I.P)
  - [x] Eating
  - ...
- Entities
  - [x] Non-Living (Minecart, Eggs...) (W.I.P)
  - [x] Entity Effects
  - [x] Players
  - [x] Mobs (W.I.P)
  - [x] Animals (W.I.P)
  - [Entity AI](https://github.com/Pumpkin-MC/Pumpkin/issues/1406)
  - [x] Boss (W.I.P)
  - [x] Villagers (W.I.P)
  - [X] Entity Saving
- Server
  - [Plugins](https://github.com/Pumpkin-MC/Pumpkin/issues/1407)
  - [x] Query
  - [x] RCON
  - [x] Inventories
  - [x] Particles
  - [x] Chat
  - [Commands](https://github.com/Pumpkin-MC/Pumpkin/issues/15)
  - [x] Permissions
  - [x] Translations
- Proxy
  - [x] Bungeecord
  - [x] Velocity

<!-- Check out our [Github Project](https://github.com/orgs/Pumpkin-MC/projects/3) to see current progress. -->

## How to run

See the upstream [Quick Start](https://docs.pumpkinmc.org/#quick-start) guide to get VerdantGolem running
(the steps are identical: clone, build with `cargo build --release`, and run the `verdantgolem` binary).

## Contributions

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md)

## Docs

Upstream Pumpkin's documentation can be found at <https://pumpkinmc.org/>

## Communication

Consider joining [the upstream Discord server](https://discord.gg/wT8XjrjKkf) to stay up-to-date on events, updates, and connect with other members.

## Funding

If you want to support the upstream project, check out the [Donation Page](https://pumpkinmc.org/donate/).

## License & Attribution

VerdantGolem is a fork of [Pumpkin](https://github.com/Pumpkin-MC/Pumpkin) and inherits its licensing:

* **Server (originally Pumpkin Server)**: Licensed under the [GNU General Public License v3.0 (GPLv3)](LICENSE).
* **Plugin API (`verdantgolem-plugin-api` & `pumpkin-plugin-wit`)**: Dual-licensed under [MIT](crates/verdantgolem-plugin-api/LICENSE-MIT) OR [Apache-2.0](crates/verdantgolem-plugin-api/LICENSE-APACHE) for maximum flexibility when writing plugins.
* **Third-Party Assets & Data**: Bedrock mappings, protocol conversion data, and Minecraft assets are subject to their respective licenses and attribution terms. See [assets/NOTICE.md](assets/NOTICE.md) for full details.
