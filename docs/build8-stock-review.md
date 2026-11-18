# Build #8：与 AOSP 安全语义的差异

审查基线：[Build #8 `370b26f`](https://github.com/sakuya-izayoi-c/TEESimulator/actions/runs/44345996534)。stock 在本文指仓库固定的 [AOSP KeyMint `cfeefc94`](https://android.googlesource.com/platform/system/keymint/+/cfeefc94bf8c4f19d19dca193d376a1fc05e8e3e/) 和公开接口契约，不是特定机型的私有 HAL。本次抽查的是证明格式、认证、密钥封装与升级路径；不是全库认证或设备测试。

## 本地改动的兼容性边界

生成模式不再把“低于 400”一律当作同一种证明格式。这里采取保守策略：不增加新的证书生成能力，只保留 #8 中已有编码、且 RootOfTrust 结构和版本号组合与公开格式相符的 `attestationVersion` 2、400、500。其余版本明确返回 `UNIMPLEMENTED`，包括合法但本地编码器未正确支持的版本。这是软件 TA 的能力限制，不是说 Android 不支持这些版本；不保证上述三版的所有扩展均已验证。这个限制只施加在生成证明扩展处，不是配置加载或所有普通密钥操作的总开关。

依据：[Android 证明格式](https://source.android.com/docs/security/features/keystore/attestation)；本地 `rust/patches/kmr-ta-versioned-rot.patch`。在与 #8 相同的 KeyMint pin 上按构建顺序应用全部补丁后，运行 `cargo test -p kmr-ta --lib` 验证参考 TA；不等价于 Android 端到端测试。

## 初审记录：三处继承自 JingMatrix 基线的差异（修改前）

1. **没有共享校验密钥时，用户认证令牌的真实性检查不等同于 stock。** `rust/patches/kmr-ta-authtoken.patch` 在这一分支仍检查绑定字段，但跳过了令牌 MAC 验证。绑定值相符和签发者可信是两个条件。AOSP 的 [`ta/src/operation.rs`](https://android.googlesource.com/platform/system/keymint/+/cfeefc94bf8c4f19d19dca193d376a1fc05e8e3e/ta/src/operation.rs#392) 要求验证；没有可用的验证能力时，不应把模拟端认作硬件认证边界。优先建议是无法验证时拒绝需要硬件用户认证的操作，或由真实硬件处理。
2. **keyblob 版本可以向后改写。** `rust/patches/kmr-ta-restamp.patch` 将高于当前安全版本的 blob 改为可重新标记；AOSP 的 [`ta/src/keys.rs`](https://android.googlesource.com/platform/system/keymint/+/cfeefc94bf8c4f19d19dca193d376a1fc05e8e3e/ta/src/keys.rs#737) 拒绝向下降级。此处指 OS/补丁级别的版本绑定，不是所有回滚保护机制。恢复 stock 语义前需明确旧 profile/keyblob 的迁移策略，否则原先还能访问的数据可能变得不可用。
3. **生成模式没有设备专属的保密根材料。** `rust/teesim-km/src/device.rs` 的 `RetrieveKeyMaterial` 使用源码中固定的值；AOSP [`ta/src/device.rs`](https://android.googlesource.com/platform/system/keymint/+/cfeefc94bf8c4f19d19dca193d376a1fc05e8e3e/ta/src/device.rs) 将设备相关的根材料交给设备实现提供。重启后可复用不等于密钥受硬件保护；真实凭据、长期密钥应留在硬件 Keystore。直接更换根材料会影响存量密钥，未作此改动。

三处文件均与上游 `f3463ed` 具有相同 Git blob，不归因于 #8 的两笔本地提交。以上结论来自静态源码，无设备、检测器或真实密钥参与。

## 后续安全修改（`fix/build8-stock-security`）

移除放宽认证令牌校验、允许 keyblob 降级的两份补丁；增量构建也会核对安全相关源码，以免复用曾应用旧补丁的目录。软件 TA 不再提供固定的根 KEK、KAK、unique-ID HBK 或 RPC HBK，没有可信后端时直接报错。旧软件 keyblob 不迁移、不解封；新的软件密钥也不能依赖旧根进行封装。这不改变真实硬件中的密钥。
