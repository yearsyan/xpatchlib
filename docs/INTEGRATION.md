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

三端各自一行依赖，无任何源码路径引用。三端 API 只有回放：`applyPatch` / `applyPatchToFile` / `algorithms` /（Android 与 C ABI 另有 `resultSize` / `xpatchlib_patch_info` 供下载前预检）。

**Android**（`app/androidApp`）：

```gradle
dependencies {
    implementation("io.github.yearsyan:xpatchlib:0.2.0")
}
```

```kotlin
val bytes = XPatch.applyPatch(patchBytes, localBundleBytes) // 校验失败抛 XPatchException

// 流式：直接在文件之间回放，内存只与补丁体积 + 固定缓冲相关（0.2.0 起）
XPatch.applyPatchToFile(patchFile.path, baseFile.path, outFile.path)
```

**iOS**（`app/iosApp`，Podfile）：

```ruby
pod 'XPatchlib', '~> 0.2'
```

```swift
let bytes = try XPatchlibApply(patch, localBundle) // C ABI，header 由 module map 暴露给 Swift

// 流式：xpatchlib_apply_file(patchPath, basePath, outPath)，校验契约相同
```

**HarmonyOS**（`app/harmonyApp`）：

```bash
ohpm install xpatchlib
```

```ets
import { applyPatch, applyPatchToFile } from 'xpatchlib';
const bytes = applyPatch(patch, localBundle);
applyPatchToFile(patchPath, basePath, outPath);   // 流式
```

**流式回放（0.2.0 起）**：`applyPatchToFile` / `xpatchlib_apply_file` / `XPatch.applyPatchToFile` 在文件之间直接回放——基包按控制流元组指定的偏移随机读、diff/extra 流按消费进度解压、结果经 64 KiB 缓冲落盘并增量哈希。峰值内存 = 补丁字节 + 几百 KB 固定缓冲，与业务包体积无关（内存版 `applyPatch` 峰值 ≈ 基包 + diff + 结果 ≈ 3 倍包体积）。校验契约不变：回放前校验基包哈希、回放后校验结果尺寸与哈希；任何失败都会删除未写完的输出文件。补丁信封本身仍在内存（工具链保证补丁显著小于全量，通常 ~6%）；`zdict` 因 zstd 字典必须连续，基包整体读入是其固有成本。

原生宿主（三端已有的 bundle 加载器）把回放嵌入升级流程：

```
查 catalog → 命中 patches[fromHash == 本机版本哈希]
  → 下载补丁（体积小，走普通存储 CDN；建议直接落盘 + 流式哈希校验）
  → XPatch.applyPatchToFile（内置双哈希校验，输出到临时文件后 rename）
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

# 鸿蒙 HAR（packaging/harmony 是完整 hvigor 工程）
packaging/harmony/build.sh              # ① ohos 静态库 + C 头文件 staged 进 module（仅回放）
cd packaging/harmony && \
  DEVECO_SDK_HOME=<DevEco sdk> hvigorw assembleHar --mode module -p product=default   # ② 产出 .har
```

ohpm 发布（一次性准备 + 每次发版）：

1. 在 https://ohpm.openharmony.cn 注册账号并完成发布者认证（签署分发协议）。
2. 本地生成发布专用密钥对（**RSA + PEM 格式 + 非空口令**，三个条件缺一不可：ohpm 控制台不接受 ed25519；CLI 要求私钥文件含 `ENCRYPTED` 标记，即传统加密 PEM——默认的 OpenSSH 格式即使有口令也会被拒）：

   ```bash
   ssh-keygen -t rsa -b 4096 -m PEM -f ~/.ohpm/ohpm_publish -C "ohpm-publish"
   # 口令必须非空；已生成的 OpenSSH 格式密钥可用 ssh-keygen -p -m PEM -f <key> 原地转换
   ```
3. 在 ohpm 网页个人中心上传 `~/.ohpm/ohpm_publish.pub` 公钥（认证管理 → 新增），并记下个人中心的 **发布码（publish_id）**。
4. 写入配置（之后 publish 不用带参数）：

   ```bash
   ohpm config set publish_id <publish_id>
   ohpm config set key_path ~/.ohpm/ohpm_publish
   # key_passphrase 拒绝明文：先用加密组件把口令加密成 security: 前缀的密文再写入
   ohpm config encrypt ~/.ohpm/crypto        # 交互输入口令，输出 security:... 密文
   ohpm config set crypto_path ~/.ohpm/crypto
   ohpm config set key_passphrase 'security:...'
   ```

5. 发布时 "contains source code" 的 WARN 属正常（HAR 以 ArkTS 源码形态分发，消费方编译；Rust/C++ 已编译为 .so），确认继续即可。

ohpm 注册表对包元数据逐项校验（顺序即报错顺序），module 目录下需齐备：

- `oh-package.json5` 的 `author` 必须含 email 或 url（对象形式：`{"name","email","url"}`）；
- `LICENSE` 非空文件（开源包强制）；
- `CHANGELOG.md` 非空文件；
- `README.md` 必须包含 `ohpm install xpatchlib` 字样的安装命令。

提交后进入人工审核，通过前 `ohpm info xpatchlib` 返回 404，可在 ohpm 个人中心查看审核状态。

5. 每次发版（CLI 在 DevEco `Contents/tools/ohpm/bin`）：

   ```bash
   ohpm prepublish packaging/harmony/xpatchlib/build/default/outputs/default/xpatchlib.har   # 先本地预检
   ohpm publish   packaging/harmony/xpatchlib/build/default/outputs/default/xpatchlib.har
   ```

   或者一次性带参数：`ohpm publish <har> --publish_id <ID> --key_path ~/.ohpm/ohpm_publish`。

CI（GitHub Actions，`.github/workflows/release.yml`）：push tag `v*` → 跑 `cargo test` → 矩阵构建五份产物（wasm/npm、Android AAR、iOS xcframework、鸿蒙 HAR）→ 产物以附件归档到 GitHub Release → 各 registry 发布（均带幂等跳过，重复 tag 不会重复发）：

| 发布渠道 | 触发开关 | 认证（repo secrets） |
|---|---|---|
| CocoaPods trunk | 默认开启 | `COCOAPODS_TRUNK_TOKEN`（来自本机 `~/.netrc`；trunk 会话约 4 个月过期，到期 `pod trunk register` 后更新） |
| Maven Central | repo 变量 `PUBLISH_MAVEN=true` | `CENTRAL_USERNAME` / `CENTRAL_PASSWORD` / `MAVEN_GPG_KEY` / `MAVEN_GPG_PASSPHRASE` |
| npm | repo 变量 `PUBLISH_NPM=true` | `NPM_TOKEN` |
| ohpm | repo 变量 `PUBLISH_OHPM=true` | `OHPM_PUBLISH_ID` / `OHPM_PRIVATE_KEY`（加密 PEM）/ `OHPM_KEY_PASSPHRASE_CIPHER`（`security:` 密文）/ `OHPM_CRYPTO_BUNDLE`（`base64 <(tar -czf - -C ~/.ohpm crypto)` 的输出） |

鸿蒙 HAR 在 CI 上的构建依赖华为 command-line tools（含 hvigor + ohpm + 完整 HarmonyOS SDK，官方仅随 DevEco 分发）：CI 从公共镜像 `pippocao/Images`（release tag `OHOS_Mac_Arm64_CommandLineTool_6.0.0`）下载 mac-arm64 包并在 `actions/cache` 里复用——这是 Tencent/BqLog 等 ohos 开源库的成熟做法。CI 的 `ci.yml` 上鸿蒙仍走 ubuntu + `openharmony-rs/setup-ohos-sdk` 的快速检查（Rust 静态库 + NAPI 编译），完整 HAR 打包只在 tag 发布时做。

`cargo publish`（xpatchlib-core → crates.io）仍为手动。

## 5. 已知边界

- **wasm 构建需要带 wasm target 的 clang**：macOS 上 Apple clang 不含 wasm，`packaging/npm/build.sh` 自动探测 Homebrew LLVM（`/opt/homebrew/opt/llvm`）；CI 建议 Linux runner，无此问题。
- **zstd 回溯窗口上限 128 MiB**：`zdict` 在"基包+新包"合计超出后远距离引用失效，比率衰减（对 JS bundle 量级无影响）。
- **bsdiff 生成峰值内存 ≈ 基包 16 倍**（int32 文本 + 后缀数组 + 类型数组）：8MB 包约 130MB，Node 工具链与服务端无压力，勿放在请求路径同步执行。
- **minify 产物对模块顺序敏感**：两次构建间模块大面积重排会拉低所有算法的比率；构建配置稳定（固定模块排序）对比率帮助最大。
- **鸿蒙 HAR 内为预编译 .so**：`hvigorw assembleHar` 在打包时用 OHOS NDK 编译 NAPI 适配层（`-DOHOS_STL=c++_static`，不携带 libc++_shared.so）并连同 Rust 静态库一起链成 `libxpatchlib_napi.so`；`src/main/cpp/types/libxpatchlib` 的 `.d.ts` 通过 `oh-package.json5` 的 `file:` 依赖打入 HAR。消费方 App 不再编译任何 C++。compatibleSdkVersion 为 `5.0.0(12)`，重编 SDK 升级时以 DevEco bundled SDK 为准。
- **feature 合并的边界情况**：`cargo build --workspace` 会因 wasm crate 开启 `produce` 而合并 feature，但三端产物脚本都是单 crate `--manifest-path` 构建，依赖图里没有 wasm，客户端产物不受影响。
