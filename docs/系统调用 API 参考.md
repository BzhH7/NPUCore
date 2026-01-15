# NPUcore-Ovo 系统调用 API 参考文档

## 1. 概述

本文档详细描述了 NPUcore-Ovo 内核实现的系统调用接口，兼容 Linux 系统调用 ABI。

### 1.1 调用约定

**RISC-V 64:**
- 系统调用号: `a7` 寄存器
- 参数: `a0` - `a5` (最多 6 个参数)
- 返回值: `a0` 寄存器
- 调用指令: `ecall`

**LoongArch 64:**
- 系统调用号: `a7` 寄存器
- 参数: `a0` - `a5`
- 返回值: `a0` 寄存器
- 调用指令: `syscall`

### 1.2 错误处理

系统调用返回负值表示错误，绝对值为错误码 (errno)。

---

## 2. 进程管理系统调用

### 2.1 exit (93)

终止当前进程。

```c
void exit(int status);
```

**参数:**
- `status`: 退出状态码

**返回值:** 不返回

---

### 2.2 exit_group (94)

终止当前线程组中的所有线程。

```c
void exit_group(int status);
```

**参数:**
- `status`: 退出状态码

**返回值:** 不返回

---

### 2.3 clone (220)

创建子进程或线程。

```c
pid_t clone(unsigned long flags, void *stack, int *parent_tid, 
            unsigned long tls, int *child_tid);
```

**参数:**
- `flags`: 克隆标志 (CloneFlags)
- `stack`: 子进程栈指针 (0 表示共享栈)
- `parent_tid`: 父线程 ID 存放位置
- `tls`: 线程本地存储
- `child_tid`: 子线程 ID 存放位置

**CloneFlags:**
```rust
bitflags! {
    pub struct CloneFlags: u32 {
        const CLONE_VM      = 0x0000_0100;  // 共享地址空间
        const CLONE_FS      = 0x0000_0200;  // 共享文件系统信息
        const CLONE_FILES   = 0x0000_0400;  // 共享文件描述符表
        const CLONE_SIGHAND = 0x0000_0800;  // 共享信号处理
        const CLONE_THREAD  = 0x0001_0000;  // 同一线程组
        const CLONE_CHILD_CLEARTID = 0x0020_0000;
        const CLONE_CHILD_SETTID   = 0x0100_0000;
    }
}
```

**返回值:**
- 成功: 父进程返回子进程 PID，子进程返回 0
- 失败: -errno

---

### 2.4 execve (221)

执行程序。

```c
int execve(const char *pathname, char *const argv[], char *const envp[]);
```

**参数:**
- `pathname`: 可执行文件路径
- `argv`: 参数数组 (以 NULL 结尾)
- `envp`: 环境变量数组 (以 NULL 结尾)

**返回值:**
- 成功: 不返回
- 失败: -errno

---

### 2.5 wait4 (260)

等待子进程状态改变。

```c
pid_t wait4(pid_t pid, int *wstatus, int options, struct rusage *rusage);
```

**参数:**
- `pid`: 目标进程 ID (-1 表示任意子进程)
- `wstatus`: 状态信息存放位置
- `options`: 等待选项 (WNOHANG, WUNTRACED 等)
- `rusage`: 资源使用信息 (可为 NULL)

**返回值:**
- 成功: 子进程 PID
- 失败: -errno

---

### 2.6 getpid (172)

获取当前进程 ID。

```c
pid_t getpid(void);
```

**返回值:** 当前进程 ID

---

### 2.7 getppid (173)

获取父进程 ID。

```c
pid_t getppid(void);
```

**返回值:** 父进程 ID

---

### 2.8 gettid (178)

获取当前线程 ID。

```c
pid_t gettid(void);
```

**返回值:** 当前线程 ID

---

### 2.9 yield (124)

主动让出 CPU。

```c
int sched_yield(void);
```

**返回值:** 总是返回 0

---

### 2.10 set_tid_address (96)

设置清除子线程 ID 的地址。

```c
pid_t set_tid_address(int *tidptr);
```

**参数:**
- `tidptr`: 线程退出时清零的地址

**返回值:** 当前线程 ID

---

## 3. 文件系统系统调用

### 3.1 openat (56)

打开文件。

```c
int openat(int dirfd, const char *pathname, int flags, mode_t mode);
```

**参数:**
- `dirfd`: 目录文件描述符 (AT_FDCWD=-100 表示当前目录)
- `pathname`: 文件路径
- `flags`: 打开标志
- `mode`: 创建文件时的权限

**OpenFlags:**
```rust
bitflags! {
    pub struct OpenFlags: u32 {
        const O_RDONLY    = 0;
        const O_WRONLY    = 1 << 0;
        const O_RDWR      = 1 << 1;
        const O_CREAT     = 1 << 6;
        const O_EXCL      = 1 << 7;
        const O_TRUNC     = 1 << 9;
        const O_APPEND    = 1 << 10;
        const O_NONBLOCK  = 1 << 11;
        const O_DIRECTORY = 1 << 16;
        const O_CLOEXEC   = 1 << 19;
    }
}
```

**返回值:**
- 成功: 文件描述符
- 失败: -errno

---

### 3.2 close (57)

关闭文件描述符。

```c
int close(int fd);
```

**参数:**
- `fd`: 文件描述符

**返回值:**
- 成功: 0
- 失败: -errno

---

### 3.3 read (63)

从文件读取数据。

```c
ssize_t read(int fd, void *buf, size_t count);
```

**参数:**
- `fd`: 文件描述符
- `buf`: 缓冲区指针
- `count`: 读取字节数

**返回值:**
- 成功: 实际读取的字节数
- 失败: -errno

---

### 3.4 write (64)

向文件写入数据。

```c
ssize_t write(int fd, const void *buf, size_t count);
```

**参数:**
- `fd`: 文件描述符
- `buf`: 数据指针
- `count`: 写入字节数

**返回值:**
- 成功: 实际写入的字节数
- 失败: -errno

---

### 3.5 lseek (62)

设置文件偏移量。

```c
off_t lseek(int fd, off_t offset, int whence);
```

**参数:**
- `fd`: 文件描述符
- `offset`: 偏移量
- `whence`: 起始位置 (SEEK_SET=0, SEEK_CUR=1, SEEK_END=2)

**返回值:**
- 成功: 新的偏移量
- 失败: -errno

---

### 3.6 dup (23) / dup3 (20)

复制文件描述符。

```c
int dup(int oldfd);
int dup3(int oldfd, int newfd, int flags);
```

**参数:**
- `oldfd`: 源文件描述符
- `newfd`: 目标文件描述符 (dup3)
- `flags`: 标志 (O_CLOEXEC)

**返回值:**
- 成功: 新文件描述符
- 失败: -errno

---

### 3.7 pipe2 (59)

创建管道。

```c
int pipe2(int pipefd[2], int flags);
```

**参数:**
- `pipefd`: 管道文件描述符数组
- `flags`: 标志 (O_NONBLOCK, O_CLOEXEC)

**返回值:**
- 成功: 0 (pipefd[0] 读端, pipefd[1] 写端)
- 失败: -errno

---

### 3.8 fstat (80) / fstatat (79)

获取文件状态。

```c
int fstat(int fd, struct stat *statbuf);
int fstatat(int dirfd, const char *pathname, struct stat *statbuf, int flags);
```

**struct stat:**
```rust
#[repr(C)]
pub struct Stat {
    pub st_dev: u64,       // 设备 ID
    pub st_ino: u64,       // inode 号
    pub st_mode: u32,      // 文件类型和权限
    pub st_nlink: u32,     // 硬链接数
    pub st_uid: u32,       // 用户 ID
    pub st_gid: u32,       // 组 ID
    pub st_rdev: u64,      // 设备 ID (特殊文件)
    pub st_size: i64,      // 文件大小
    pub st_blksize: i64,   // 块大小
    pub st_blocks: i64,    // 块数
    pub st_atime: TimeSpec, // 访问时间
    pub st_mtime: TimeSpec, // 修改时间
    pub st_ctime: TimeSpec, // 状态变更时间
}
```

**返回值:**
- 成功: 0
- 失败: -errno

---

### 3.9 mkdirat (34)

创建目录。

```c
int mkdirat(int dirfd, const char *pathname, mode_t mode);
```

**返回值:**
- 成功: 0
- 失败: -errno

---

### 3.10 unlinkat (35)

删除文件或目录。

```c
int unlinkat(int dirfd, const char *pathname, int flags);
```

**flags:**
- `AT_REMOVEDIR` (0x200): 删除目录

**返回值:**
- 成功: 0
- 失败: -errno

---

### 3.11 getdents64 (61)

读取目录项。

```c
ssize_t getdents64(int fd, void *dirp, size_t count);
```

**struct linux_dirent64:**
```rust
#[repr(C)]
pub struct Dirent {
    pub d_ino: u64,        // inode 号
    pub d_off: i64,        // 下一个 dirent 的偏移
    pub d_reclen: u16,     // 记录长度
    pub d_type: u8,        // 文件类型
    pub d_name: [u8; 0],   // 文件名 (变长)
}
```

**返回值:**
- 成功: 读取的字节数
- 失败: -errno

---

### 3.12 getcwd (17)

获取当前工作目录。

```c
char *getcwd(char *buf, size_t size);
```

**返回值:**
- 成功: buf 指针
- 失败: NULL (errno 设置)

---

### 3.13 chdir (49)

改变当前工作目录。

```c
int chdir(const char *path);
```

**返回值:**
- 成功: 0
- 失败: -errno

---

## 4. 内存管理系统调用

### 4.1 brk (214)

设置程序数据段结束位置。

```c
int brk(void *addr);
```

**参数:**
- `addr`: 新的数据段结束地址 (0 表示查询当前值)

**返回值:**
- 成功: 新的数据段结束地址
- 失败: -errno

---

### 4.2 mmap (222)

创建内存映射。

```c
void *mmap(void *addr, size_t length, int prot, int flags, int fd, off_t offset);
```

**参数:**
- `addr`: 建议的映射地址 (可为 NULL)
- `length`: 映射长度
- `prot`: 保护标志 (PROT_READ, PROT_WRITE, PROT_EXEC)
- `flags`: 映射标志 (MAP_SHARED, MAP_PRIVATE, MAP_ANONYMOUS, MAP_FIXED)
- `fd`: 文件描述符 (匿名映射时为 -1)
- `offset`: 文件偏移

**保护标志:**
```c
#define PROT_NONE   0x0
#define PROT_READ   0x1
#define PROT_WRITE  0x2
#define PROT_EXEC   0x4
```

**映射标志:**
```c
#define MAP_SHARED    0x01
#define MAP_PRIVATE   0x02
#define MAP_FIXED     0x10
#define MAP_ANONYMOUS 0x20
```

**返回值:**
- 成功: 映射的起始地址
- 失败: MAP_FAILED (-1)

---

### 4.3 munmap (215)

取消内存映射。

```c
int munmap(void *addr, size_t length);
```

**返回值:**
- 成功: 0
- 失败: -errno

---

### 4.4 mprotect (226)

修改内存保护属性。

```c
int mprotect(void *addr, size_t len, int prot);
```

**返回值:**
- 成功: 0
- 失败: -errno

---

## 5. 信号系统调用

### 5.1 sigaction (134)

设置信号处理函数。

```c
int sigaction(int signum, const struct sigaction *act, struct sigaction *oldact);
```

**struct sigaction:**
```rust
#[repr(C)]
pub struct SigAction {
    pub sa_handler: usize,    // 信号处理函数
    pub sa_flags: u32,        // 标志
    pub sa_restorer: usize,   // 恢复函数
    pub sa_mask: u64,         // 信号掩码
}
```

**返回值:**
- 成功: 0
- 失败: -errno

---

### 5.2 sigprocmask (135)

修改信号掩码。

```c
int sigprocmask(int how, const sigset_t *set, sigset_t *oldset);
```

**how 参数:**
- `SIG_BLOCK` (0): 添加到掩码
- `SIG_UNBLOCK` (1): 从掩码移除
- `SIG_SETMASK` (2): 设置掩码

**返回值:**
- 成功: 0
- 失败: -errno

---

### 5.3 kill (129)

向进程发送信号。

```c
int kill(pid_t pid, int sig);
```

**返回值:**
- 成功: 0
- 失败: -errno

---

### 5.4 sigreturn (139)

从信号处理函数返回。

```c
int sigreturn(void);
```

**返回值:** 不返回（恢复原上下文）

---

## 6. 时间系统调用

### 6.1 clock_gettime (113)

获取时钟时间。

```c
int clock_gettime(clockid_t clockid, struct timespec *tp);
```

**clockid:**
- `CLOCK_REALTIME` (0): 系统实时时钟
- `CLOCK_MONOTONIC` (1): 单调时钟

**struct timespec:**
```rust
#[repr(C)]
pub struct TimeSpec {
    pub tv_sec: i64,   // 秒
    pub tv_nsec: i64,  // 纳秒
}
```

**返回值:**
- 成功: 0
- 失败: -errno

---

### 6.2 nanosleep (101)

高精度睡眠。

```c
int nanosleep(const struct timespec *req, struct timespec *rem);
```

**参数:**
- `req`: 请求的睡眠时间
- `rem`: 剩余时间 (被信号中断时)

**返回值:**
- 成功: 0
- 被中断: -EINTR

---

### 6.3 gettimeofday (169)

获取当前时间。

```c
int gettimeofday(struct timeval *tv, struct timezone *tz);
```

**返回值:**
- 成功: 0
- 失败: -errno

---

## 7. 网络系统调用

### 7.1 socket (198)

创建套接字。

```c
int socket(int domain, int type, int protocol);
```

**参数:**
- `domain`: AF_INET (2), AF_INET6 (10), AF_UNIX (1)
- `type`: SOCK_STREAM (1), SOCK_DGRAM (2)
- `protocol`: 通常为 0

**返回值:**
- 成功: 套接字文件描述符
- 失败: -errno

---

### 7.2 bind (200)

绑定地址。

```c
int bind(int sockfd, const struct sockaddr *addr, socklen_t addrlen);
```

**返回值:**
- 成功: 0
- 失败: -errno

---

### 7.3 listen (201)

监听连接。

```c
int listen(int sockfd, int backlog);
```

**返回值:**
- 成功: 0
- 失败: -errno

---

### 7.4 accept (202)

接受连接。

```c
int accept(int sockfd, struct sockaddr *addr, socklen_t *addrlen);
```

**返回值:**
- 成功: 新的套接字文件描述符
- 失败: -errno

---

### 7.5 connect (203)

建立连接。

```c
int connect(int sockfd, const struct sockaddr *addr, socklen_t addrlen);
```

**返回值:**
- 成功: 0
- 失败: -errno

---

### 7.6 sendto (206) / recvfrom (207)

发送/接收数据。

```c
ssize_t sendto(int sockfd, const void *buf, size_t len, int flags,
               const struct sockaddr *dest_addr, socklen_t addrlen);
ssize_t recvfrom(int sockfd, void *buf, size_t len, int flags,
                 struct sockaddr *src_addr, socklen_t *addrlen);
```

**返回值:**
- 成功: 发送/接收的字节数
- 失败: -errno

---

## 8. 同步系统调用

### 8.1 futex (98)

快速用户空间互斥锁。

```c
int futex(int *uaddr, int op, int val, const struct timespec *timeout,
          int *uaddr2, int val3);
```

**操作码 (op):**
- `FUTEX_WAIT` (0): 等待
- `FUTEX_WAKE` (1): 唤醒
- `FUTEX_REQUEUE` (3): 重新排队

**返回值:**
- 成功: 依操作而定
- 失败: -errno

---

## 9. 错误码参考

| 错误码 | 值 | 描述 |
|--------|-----|------|
| EPERM | 1 | 操作不允许 |
| ENOENT | 2 | 文件不存在 |
| ESRCH | 3 | 进程不存在 |
| EINTR | 4 | 系统调用被中断 |
| EIO | 5 | I/O 错误 |
| ENXIO | 6 | 设备不存在 |
| E2BIG | 7 | 参数列表过长 |
| EBADF | 9 | 无效的文件描述符 |
| ECHILD | 10 | 没有子进程 |
| EAGAIN | 11 | 资源暂时不可用 |
| ENOMEM | 12 | 内存不足 |
| EACCES | 13 | 权限不足 |
| EFAULT | 14 | 地址错误 |
| EEXIST | 17 | 文件已存在 |
| ENOTDIR | 20 | 不是目录 |
| EISDIR | 21 | 是目录 |
| EINVAL | 22 | 无效参数 |
| EMFILE | 24 | 打开文件过多 |
| ENOSPC | 28 | 磁盘空间不足 |
| ESPIPE | 29 | 非法 seek |
| EPIPE | 32 | 管道破裂 |
| ERANGE | 34 | 结果超出范围 |
| ENOSYS | 38 | 系统调用未实现 |

---

*文档版本: 1.0*  
*最后更新: 2026年1月*
