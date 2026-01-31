# System Utilities (系统工具集)

一组轻量级系统工具，用于演示操作系统内核的各种能力。

## 工具列表

| 工具 | 描述 | 演示的内核功能 |
|------|------|----------------|
| `cat` | 显示文件内容 | open, read, write, close |
| `echo` | 输出文本 | write, 命令行参数处理 |
| `wc` | 统计行数/单词/字节 | read, 文件I/O |
| `tree` | 树形显示目录 | opendir, readdir, stat |
| `cal` | 显示日历 | gettimeofday, localtime |
| `hexdump` | 十六进制显示文件 | read, 格式化输出 |
| `uptime` | 显示系统运行时间 | gettimeofday |
| `ls` | 列出目录内容 | opendir, readdir, stat, lstat |
| `pwd` | 显示当前目录 | getcwd |
| `mkdir` | 创建目录 | mkdir, mkdirat |
| `rm` | 删除文件/目录 | unlink, unlinkat, rmdir |
| `cp` | 复制文件 | open, read, write, stat |
| `mv` | 移动/重命名文件 | rename, open, read, write |
| `touch` | 创建/更新文件 | open, stat |
| `top` | 系统信息监控 | sysinfo, time |

## 编译

```bash
make        # 编译所有工具（两个架构）
make clean  # 清理
```

## 使用示例

```bash
# cat - 显示文件内容
cat /etc/passwd
cat file1.txt file2.txt

# echo - 输出文本
echo Hello, World!
echo -n "No newline"

# wc - 统计文件
wc myfile.txt
wc -l myfile.txt  # 只统计行数

# tree - 显示目录树
tree /
tree /home

# cal - 日历
cal          # 当前月份
cal 2024     # 2024年整年
cal 3 2024   # 2024年3月

# hexdump - 十六进制查看
hexdump /bin/cat
hexdump myfile.bin

# uptime - 系统运行时间
uptime

# ls - 列出目录
ls           # 简单列表
ls -l        # 详细列表
ls -la       # 包含隐藏文件
ls -lh       # 人类可读的大小

# pwd - 显示当前目录
pwd

# mkdir - 创建目录
mkdir mydir
mkdir -p path/to/dir  # 递归创建

# rm - 删除文件/目录
rm file.txt
rm -r dir     # 递归删除目录
rm -rf dir    # 强制删除，不提示

# cp - 复制文件
cp file1 file2
cp -r dir1 dir2  # 递归复制目录

# mv - 移动/重命名
mv old new
mv file dir/

# touch - 创建/更新文件
touch newfile.txt
touch -c existingfile  # 只更新时间，不创建

# top - 系统监控
top           # 持续显示
top -n 1      # 只显示一次
top -d 5      # 5秒刷新间隔
```

## 安装位置

所有工具编译后会被安装到文件系统的 `/bin` 目录。系统的 PATH 环境变量已配置为包含 `/bin`，因此可以直接使用命令名运行（如 `ls`），无需指定完整路径（如 `/bin/ls`）。

## 内核功能依赖

这些工具依赖以下系统调用:

- 文件操作: `open`, `close`, `read`, `write`, `stat`, `fstat`
- 目录操作: `opendir`/`getdents64`, `readdir`
- 时间: `gettimeofday`, `clock_gettime`
- 内存: `brk`/`mmap` (用于libc的malloc)
