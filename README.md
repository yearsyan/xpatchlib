# xpatchlib

热更业务包的确定性二进制差量补丁库：一个 Rust 核心，五端产物。**生成在工具链/服务端（Node/wasm），Android / iOS / HarmonyOS客户端只回放**

## 安装

| 平台 | 坐标 |
|---|---|
| Node 工具链（生成 + 回放） | `npm i -D @lynfe/xpatchlib` |
| Android（仅回放） | `implementation("io.github.yearsyan:xpatchlib:x.y.z")` |
| iOS（仅回放） | `pod 'XPatchlib'` |
| HarmonyOS（仅回放） | `ohpm install xpatchlib` |

接入与发布设计见 [docs/INTEGRATION.md](docs/INTEGRATION.md)。

## License

MIT
