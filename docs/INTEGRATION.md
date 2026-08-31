# 发布与接入设计

> 原则：**template/apps 与 xpatchlib 源码之间零位置耦合**。三端 app 和工具链只消费各生态包管理器里的版本化产物；xpatchlib 单独建仓、单独发版，任何 app 升级 xpatchlib 就是一次依赖版本号的变更。
>
> 职责边界：**生成在工具链/服务端，回放在客户端**。三端产物全部以 `default-features = false` 构建 core，补丁生产代码（后缀数组、匹配器、zstd 压缩器、`create` 入口）不进任何客户端二进制。

## 1. 产物矩阵：一个 tag，五份产物

| 消费方 | 产物 | 仓库 | 接入方式 |
|---|---|---|---|
| Rust（服务端/CLI） | `xpatchlib-core` crate | crates.io | `Cargo.toml` 依赖 |
| template/packages 工具链 | `@lynfe/xpatchlib`（wasm，生成 + 回放） | npm | pnpm devDependency |
| template/apps Android | `io.github.yearsyan:xpatchlib`（AAR，内置四 ABI so，仅回放） | Maven Central（`io.github.yearsyan`） | `implementation("io.github.yearsyan:xpatchlib:x.y.z")` |
| template/apps iOS | `XPatchlib`（Pod + xcframework，仅回放） | CocoaPods trunk | `pod 'XPatchlib', '~> x.y.z'` |
| template/apps HarmonyOS | `xpatchlib`（HAR + NAPI so，仅回放） | ohpm 公共仓库 | `ohpm install xpatchlib` |

版本策略：

- **一个 git tag（如 `v0.2.0`）驱动全部产物**，五份产物版本号一致，CI 各自打标签发布。
- 各生态只升级自己关心的产物；envelope 自带格式版本（`XPDL v1`）与算法名注册表，**老客户端永远能回放新工具链产的补丁**（只要算法名还编译在客户端里），格式破坏性变更才升 envelope 版本。
- 建议算法集合做加法不做减法：新增算法（如未来的 HDiffPatch）直接追加；移除算法必须先在服务端停发该算法的补丁、等客户端覆盖率回落再发版。

## 2. Node 工具链接入（补丁生成侧）

bundle 上传流程在发布新版本时由 Node 工具链生成补丁，devDependency 引入即可：

```jsonc
// template/packages/toolchain/package.json
"devDependencies": {
  "@lynfe/xpatchlib": "^0.1.0"
}
```

上传流程挂一段"差量准备"（伪代码）：

```ts
import { createPatch, patchInfo } from "@lynfe/xpatchlib";

// snapshot = 上一步拿到的最近 N 个已发布版本（bundle-server 已有该接口）
for (const previous of snapshot.versions.slice(0, 5)) {
  for (const algorithm of ["bsdiff", "zdict"]) {
    const patch = createPatch(algorithm, previous.bundleBytes, next.bundleBytes);
    if (patch.length >= fullCompressedSize) continue; // 打不过全量就不发
    await uploadArtifact({
      kind: "delta-patch",
      fromHash: previous.hash,        // 客户端按本机版本哈希精确匹配
      algorithm,
      data: patch,
      info: patchInfo(patch),
    });
  }
}
```

要点：

- **生成完全离线于发布路径**：可以先做成独立子命令（`patch --from <version>` 风格），跑通后再并入 upload；bundle-server 只多收一种 artifact。
- 补丁字节确定 → 内容寻址存储天然去重，重跑无副作用。
- `bsdiff` + `zdict` 双发，目录同时暴露，客户端按设备能力选；`full` 基线在工具链侧用来决定"这次不发差量"。

## 3. App 三端接入（回放侧）

三端各自一行依赖，无任何源码路径引用。三端 API 只有回放：`applyPatch` / `algorithms` /（Android 与 C ABI 另有 `resultSize` / `xpatchlib_patch_info` 供下载前预检）。

**Android**（`app/androidApp`）：

```gradle
dependencies {
    implementation("io.github.yearsyan:xpatchlib:0.1.0")
}
```

```kotlin
val bytes = XPatch.applyPatch(patchBytes, localBundleBytes) // 校验失败抛 XPatchException
```

**iOS**（`app/iosApp`，Podfile）：

```ruby
pod 'XPatchlib', '~> 0.1'
```

```swift
let bytes = try XPatchlibApply(patch, localBundle) // C ABI，header 由 module map 暴露给 Swift
```

**HarmonyOS**（`app/harmonyApp`）：

```bash
ohpm install xpatchlib
```

```ets
import { applyPatch } from 'xpatchlib';
const bytes = applyPatch(patch, localBundle);
```

原生宿主（三端已有的 bundle 加载器）把回放嵌入升级流程：

```
查 catalog → 命中 patches[fromHash == 本机版本哈希]
  → 下载补丁（体积小，走普通存储 CDN）
  → XPatch.applyPatch（内置双哈希校验）
  → 成功：写入新版本；失败：丢弃，无条件回退全量下载
  → 未命中：直接全量
```

回放产物与全量下载字节一致，后续校验/落盘逻辑无需感知补丁存在。

## 4. 本仓库的构建与发布

```bash
# 日常验证
cargo test --workspace
cargo clippy --workspace
cargo check -p xpatchlib-ffi -p xpatchlib-jni   # 回放侧无生产代码，可独立编译

# 产物构建（各脚本自带依赖检查与提示；均为单 crate 构建，产物保证纯回放）
packaging/npm/build.sh                  # wasm → packaging/npm/wasm/（生成 + 回放）
packaging/android/build-aar.sh          # 需 ANDROID_NDK_HOME + JDK（仅回放）
packaging/ios/build-xcframework.sh      # 需 Xcode（仅回放）
packaging/harmony/build.sh              # 需 OHOS_NDK_HOME + rustup ohos target（仅回放）
```

CI（GitHub Actions，`.github/workflows/release.yml`）：push tag `v*` → 跑 `cargo test` → 矩阵构建产物（wasm/npm、Android AAR、iOS xcframework、鸿蒙 ohos 静态库——经 openharmony-rs/setup-ohos-sdk 拉取 OHOS NDK）→ 产物以附件归档到 GitHub Release → **自动 `pod trunk push`**（CI 对自己构建的 zip 计算 sha256、按 tag 渲染 podspec 后推送；认证用仓库 secret `COCOAPODS_TRUNK_TOKEN`，来自本机 `~/.netrc`，trunk 会话约 4 个月过期，到期需重新 `pod trunk register` 并更新 secret）。其余 registry 发布另行接入：`cargo publish`（xpatchlib-core）、`npm publish`（packaging/npm，wasm 产物随包发布）、AAR 推 Maven Central（namespace `io.github.yearsyan`）、HAR 推 ohpm。

## 5. 已知边界

- **wasm 构建需要带 wasm target 的 clang**：macOS 上 Apple clang 不含 wasm，`packaging/npm/build.sh` 自动探测 Homebrew LLVM（`/opt/homebrew/opt/llvm`）；CI 建议 Linux runner，无此问题。
- **zstd 回溯窗口上限 128 MiB**：`zdict` 在"基包+新包"合计超出后远距离引用失效，比率衰减（对 JS bundle 量级无影响）。
- **bsdiff 生成峰值内存 ≈ 基包 16 倍**（int32 文本 + 后缀数组 + 类型数组）：8MB 包约 130MB，Node 工具链与服务端无压力，勿放在请求路径同步执行。
- **minify 产物对模块顺序敏感**：两次构建间模块大面积重排会拉低所有算法的比率；构建配置稳定（固定模块排序）对比率帮助最大。
- **鸿蒙 NAPI 适配层为源码形态**（HAR 内置 CMake 源编）：OHOS NDK 的 ABI 与 rustc `aarch64-unknown-linux-ohos` 均在快速演进，源编比预编 so 更抗工具链漂移；`build.sh` 负责产出静态库。
- **feature 合并的边界情况**：`cargo build --workspace` 会因 wasm crate 开启 `produce` 而合并 feature，但三端产物脚本都是单 crate `--manifest-path` 构建，依赖图里没有 wasm，客户端产物不受影响。
