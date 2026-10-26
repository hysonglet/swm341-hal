# swm341-hal

Synwit（华芯微特）**SWM341** 系列微控制器的硬件抽象层（HAL），基于 `swm341-pac`。

SWM341 采用 Arm 中国 **STAR-MC1** 内核——Armv8-M Mainline，带 FPU 与 MPU，二进制兼容
Cortex-M33——因此编译目标是 `thumbv8m.main-none-eabihf`。

## 特性

- `no_std` / 裸机嵌入式，使用 `embedded-hal` 1.0 与 `embedded-hal-async` 1.0 约定
- 驱动泛型于 [`mode::Mode`]：`Blocking` 忙等，`Async` 异步（`embassy` feature 开启）
- 自带中断向量表：PAC 的 SVD 只描述 NVIC 向量 0..=28，芯片实际有 91 个，本 crate 用
  `vector_table` / `interrupt` 补全

## 支持的外设

以下模块均已实现驱动（大多泛型于 `Mode`，在 `embassy` feature 下提供异步 API）：

`adc` · `btimr` · `can` · `cmp` · `cordic` · `crc` · `dac` · `div` · `dma` ·
`dma2d` · `dwt` · `flash` · `gpio` · `i2c` · `jpeg` · `lcdc` · `opa` · `pwm` ·
`qei` · `rtc` · `sdio` · `sdramc` · `sfc` · `spi` · `timr` · `uart` · `wdt`

此外 [`clock`] 管理时钟树与外设时钟门，[`gpio`] 管理引脚矩阵，`mcu` 提供外设单例，
[`init`] 一次性发放它们。

## 快速开始

```rust
use swm341_hal as hal;

#[cortex_m_rt::entry]
fn main() -> ! {
    // 150 MHz 内部高速时钟；板级通常还需初始化 SDRAM 才可用作堆/帧缓冲。
    let p = hal::init(hal::Config {
        clock: hal::clock::ClockConfig::hrc_150mhz(),
    });

    let mut led = hal::gpio::Output::new(
        p.PB0,
        hal::gpio::OutputConfig::default(),
    );
    loop {
        led.toggle();
    }
}
```

## Cargo features

| feature  | 默认 | 说明 |
|----------|------|------|
| `embassy` | 是   | 启用异步驱动模式，以及 `embassy-executor` / `embassy-time`（SysTick 1 kHz 驱动）支持。关闭后只保留 `Blocking` API。 |

## 示例

`examples/` 下每个外设都有可直接烧录的演示，可分为两类：

**Blocking（裸机 `main`，`cortex-m-rt`）**

| 示例 | 说明 |
|------|------|
| `hello_rtt` | RTT 打印的最小模板 |
| `clock_block` / `dwt_block` | 时钟树 / DWT 计时器 |
| `spi_block` / `spi_slave` / `i2c_block` / `i2c_slave` | 同步 SPI / I2C 主从 |
| `timr_block` / `pwm_block` / `adc_block` / `cmp_block` / `qei_block` / `wdt_block` | 定时器 / PWM / ADC / 模拟比较 / 正交编码 / 看门狗 |
| `flash_block` | 内部 Flash 擦写 |
| `sfc_flash_id` / `sfc_block` | 外部 SPI NOR：读 ID、擦/写/读校验 |
| `sfc_xip` | 外部 Flash 的 XiP：数据映射读取 + 就地执行机器码 |
| `sdramc_block` | SDRAM 控制器 |
| `lcdc_block` / `lcdc_gradient` | TFT-LCD 并行 RGB 点亮与渐变动画 |

**Async（`embassy_executor::main`，需 `embassy` feature）**

| 示例 | 说明 |
|------|------|
| `embassy_gpio_blink` / `embassy_gpio_exti` | GPIO 翻转 / 外部中断 |
| `embassy_spi` / `embassy_spi_slave` / `spi_dual` | 异步 SPI 主从 / 双线 SPI |
| `embassy_i2c` / `embassy_i2c_slave` | 异步 I2C 主从 |
| `embassy_timr` / `embassy_pwm` / `embassy_adc` / `embassy_uart` | 异步定时 / PWM / ADC / 串口 |
| `embassy_lcdc` | 异步 LCD 动画：1 Hz 切换水平/垂直渐变 |

运行某个示例（默认目标已在 `.cargo/config.toml` 固定）：

```sh
cargo run --example hello_rtt
cargo run --example embassy_lcdc   # 需要 embassy feature（默认开启）
```

## 构建与运行

- `.cargo/config.toml` 已固定 `target = thumbv8m.main-none-eabihf`，并配置
  `runner = "probe-rs run --chip swm341"`，因此 `cargo run` 会烧录并通过 RTT 输出
  `rprintln!` 的内容（示例多用此通道打印）。
- 主机单元测试（仅覆盖不涉及硬件的算术与参数检查，因为 `no_std` 目标没有 `test` crate）：

  ```sh
  cargo test --target aarch64-apple-darwin --lib
  ```

## 内存与向量表

- `memory.x`：SRAM / Flash 布局，按板级修改即可。
- `device.x`：补全 PAC 缺失的 89 个中断向量。**不要**开启 PAC 自身的 `rt` feature，
  否则会注入一份更短的向量表覆盖它。

## 参考

- 本 HAL 的不少驱动与示例移植自华芯微特厂商库（`SWM341_Lib`，含 `SWM341_sfc.c`、
  各类 `Simpl*` 例程与应用笔记）及《SWM341 数据手册》。这些是**本地参考材料，如需查阅请自行从华芯微特官方渠道获取
  对应版本的厂商库与数据手册。

## License

[0BSD](https://opensource.org/licenses/0BSD)。
