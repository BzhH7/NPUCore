# Utils 工具集演示说明

## 快速开始

在系统启动后的 shell 中，运行以下命令来查看所有工具的演示：

```bash
sh /bin/demo.sh
```

或者直接运行（如果 /bin 在 PATH 中）：

```bash
demo.sh
```

## 可用工具列表

| 命令 | 功能说明 | 示例用法 |
|------|----------|----------|
| `pwd` | 显示当前工作目录 | `pwd` |
| `ls` | 列出目录内容 | `ls`, `ls -l` |
| `echo` | 输出文本到终端 | `echo "Hello World"` |
| `touch` | 创建空文件 | `touch /tmp/newfile.txt` |
| `cat` | 显示文件内容 | `cat /tmp/file.txt` |
| `cp` | 复制文件 | `cp /tmp/src.txt /tmp/dst.txt` |
| `mv` | 移动/重命名文件 | `mv /tmp/old.txt /tmp/new.txt` |
| `mkdir` | 创建目录 | `mkdir /tmp/mydir` |
| `rm` | 删除文件 | `rm /tmp/file.txt` |
| `tree` | 显示目录树结构 | `tree /tmp` |
| `wc` | 统计文件字数/行数 | `wc /tmp/file.txt` |
| `hexdump` | 十六进制查看文件 | `hexdump /tmp/file.txt` |
| `cal` | 显示日历 | `cal` |
| `uptime` | 显示系统运行时间 | `uptime` |
| `top` | 系统资源监控工具 | `top` (按 q 退出) |

## 手动测试示例

### 基础文件操作
```bash
# 创建测试目录
mkdir /tmp/test
cd /tmp/test

# 创建文件
touch file1.txt
echo "Hello from echo!" > file2.txt

# 查看文件
ls -l
cat file2.txt

# 复制和移动
cp file1.txt file1_copy.txt
mv file1_copy.txt renamed.txt

# 清理
cd /tmp
rm -r test
```

### 系统监控
```bash
# 查看系统运行时间
uptime

# 查看日历
cal

# 实时监控系统资源（按 q 退出）
top
```

### 配合游戏测试 CPU 使用率
```bash
# 在后台运行游戏来产生 CPU 负载
# 然后在 top 中观察 CPU 使用率变化

# 运行俄罗斯方块
tetris &

# 运行 top 查看 CPU 使用率
top

# 运行更多游戏来增加负载
snake &
2048 &
top
```

## 演示脚本说明

`demo.sh` 会逐个演示所有工具的基本用法：

1. 每个工具演示后会暂停，按 Enter 继续
2. 脚本会自动创建测试文件和目录
3. 演示结束后会自动清理测试文件
4. 使用颜色高亮显示命令和分组

## 构建说明

工具集会在构建内核时自动复制到文件系统的 `/bin` 目录：

- **LoongArch**: `make la64-b-only` 或 `make la64-b-run`
- **RISC-V**: `make -f make/rv64.mk all` 或 `make -f make/rv64.mk run`

demo.sh 脚本会在 buildfs.sh 执行时自动复制到目标文件系统。
