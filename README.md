## OSKernel2025-NPUcore-Ovo (RISC-V + LoongArch)

### 一、简介

`NPUcore-Ovo` 基于 `NPUcore-BLOSSOM` 框架，参考借鉴去年内核赛道优秀参赛队伍与 `Linux` 内核的诸多优秀设计，完善其内部功能实现并进行迭代升级而形成的竞赛操作系统。
<div align="center">

# OSKernel2025-NPUcore-Ovo

### RISC-V + LoongArch 双架构操作系统内核

[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-nightly--2024--05--01-orange.svg)](https://www.rust-lang.org/)
[![RISC-V](https://img.shields.io/badge/RISC--V-rv64gc-green.svg)](https://riscv.org/)
[![LoongArch](https://img.shields.io/badge/LoongArch-la64-red.svg)](https://loongson.cn/)

</div>

---

## 目录

- [简介](#-简介)
- [特性](#-特性)
- [初赛完成情况](#-初赛完成情况)
- [分支介绍](#-分支介绍)
- [快速开始](#-快速开始)
- [演示视频](#-演示视频)
- [参考资料](#-参考资料)

---

## 简介

**NPUcore-Ovo** 是基于 `NPUcore-BLOSSOM` 框架开发的竞赛操作系统内核，参考借鉴了往届内核赛道优秀参赛队伍与 `Linux` 内核的诸多优秀设计，完善其内部功能实现并进行迭代升级而形成。

> **目标**: 参与全国大学生计算机系统能力大赛 - 操作系统设计赛

---

## 特性

| 特性 | 描述 |
|:---:|:---|
| **双架构支持** | 同时支持 RISC-V 64 和 LoongArch 64 架构 |
| **多平台适配** | QEMU 模拟器 / VisionFive2 / 龙芯 2K1000 |
| **文件系统** | FAT32 / EXT4 双文件系统支持 |
| **网络协议栈** | 基于 smoltcp 的 TCP/UDP 支持 |
| **内存管理** | 写时复制 (CoW) / ZRAM 压缩 / Swap 交换 |
| **进程管理** | 多进程/多线程 / 信号处理 / Futex /多 核|

---

## 初赛完成情况

### 1️ RISC-V (VisionFive 2) ✅ 全部通过

<details>
<summary> 点击展开详细测试结果</summary>

| 测试样例 | 通过 | 总数 | 状态 |
|:---------|:----:|:----:|:----:|
| test_execve | 3 | 3 | ✅ |
| test_open | 3 | 3 | ✅ |
| test_getdents | 5 | 5 | ✅ |
| test_gettimeofday | 3 | 3 | ✅ |
| test_munmap | 4 | 4 | ✅ |
| test_yield | 4 | 4 | ✅ |
| test_getpid | 3 | 3 | ✅ |
| test_mount | 5 | 5 | ✅ |
| test_dup | 2 | 2 | ✅ |
| test_waitpid | 4 | 4 | ✅ |
| test_write | 2 | 2 | ✅ |
| test_close | 2 | 2 | ✅ |
| test_exit | 2 | 2 | ✅ |
| test_times | 6 | 6 | ✅ |
| test_read | 3 | 3 | ✅ |
| test_getppid | 2 | 2 | ✅ |
| test_clone | 4 | 4 | ✅ |
| test_openat | 4 | 4 | ✅ |
| test_mmap | 3 | 3 | ✅ |
| test_fork | 3 | 3 | ✅ |
| test_sleep | 2 | 2 | ✅ |
| test_mkdir | 3 | 3 | ✅ |
| test_umount | 5 | 5 | ✅ |
| test_chdir | 3 | 3 | ✅ |
| test_unlink | 2 | 2 | ✅ |
| test_fstat | 3 | 3 | ✅ |
| test_pipe | 4 | 4 | ✅ |
| test_getcwd | 2 | 2 | ✅ |
| test_dup2 | 2 | 2 | ✅ |
| test_brk | 3 | 3 | ✅ |
| test_uname | 2 | 2 | ✅ |
| test_wait | 4 | 4 | ✅ |

**总计: 32/32 测试用例通过 (100%)**

</details>

### 2️ LoongArch (2K1000) ✅ 全部通过

<details>
<summary> 点击展开详细测试结果</summary>

| 测试样例 | 通过 | 总数 | 状态 |
|:---------|:----:|:----:|:----:|
| test_dup | 2 | 2 | ✅ |
| test_uname | 2 | 2 | ✅ |
| test_dup2 | 2 | 2 | ✅ |
| test_execve | 3 | 3 | ✅ |
| test_pipe | 4 | 4 | ✅ |
| test_getppid | 2 | 2 | ✅ |
| test_chdir | 3 | 3 | ✅ |
| test_wait | 4 | 4 | ✅ |
| test_munmap | 4 | 4 | ✅ |
| test_fstat | 3 | 3 | ✅ |
| test_getpid | 3 | 3 | ✅ |
| test_exit | 2 | 2 | ✅ |
| test_read | 3 | 3 | ✅ |
| test_mkdir | 3 | 3 | ✅ |
| test_sleep | 2 | 2 | ✅ |
| test_times | 6 | 6 | ✅ |
| test_clone | 4 | 4 | ✅ |
| test_mmap | 3 | 3 | ✅ |
| test_fork | 3 | 3 | ✅ |
| test_write | 2 | 2 | ✅ |
| test_close | 2 | 2 | ✅ |
| test_openat | 4 | 4 | ✅ |
| test_brk | 3 | 3 | ✅ |
| test_mount | 5 | 5 | ✅ |
| test_getcwd | 2 | 2 | ✅ |
| test_umount | 5 | 5 | ✅ |
| test_unlink | 2 | 2 | ✅ |
| test_gettimeofday | 3 | 3 | ✅ |
| test_yield | 4 | 4 | ✅ |
| test_open | 3 | 3 | ✅ |
| test_getdents | 5 | 5 | ✅ |
| test_waitpid | 4 | 4 | ✅ |

**总计: 32/32 测试用例通过 (100%)**

</details>

---

## 分支介绍

| 分支 | 描述 | 状态 |
|:-----|:-----|:----:|
| `main` | 最新的默认分支，支持 2025 年 RV 和 LA 架构下的测试 |
| `comp_rv64` | 用于初赛 VisionFive 2 提测的分支 |
| `comp_la64` | 用于初赛 2K1000 提测的分支 |

---

## 快速开始

### 环境配置

```bash
# 使用 Docker 环境 (推荐)
docker run -it --privileged \
    -v "$(pwd):/root/workspace" \
    -w /root/workspace \
    docker.educg.net/cg/os-contest:2024p8.3 /bin/bash
```

### 构建运行

```bash
# 构建
make all

# 运行
make run
```

> 📚 详细文档请参阅 [docs/](./docs/) 目录

---

## 演示视频

### 初赛演示视频

通过网盘分享的文件：演示视频
链接: [演示视频](https://pan.baidu.com/s/1u2YaH5kAJJENULcIbV7n1g) 提取码: ce6p

---

## 参考资料

- 📖 [NPUcore-BLOSSOM](https://gitlab.eduxiji.net/T202510699995278/oskernel2025-npucore-blossom/) - 基础框架
- 📖 [RocketOS](https://github.com/RocketOS) - 参考实现
- 📖 [rCore-Tutorial](https://rcore-os.github.io/rCore-Tutorial-Book-v3/) - 教程参考
