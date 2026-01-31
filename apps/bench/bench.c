/**
 * Benchmark Suite - 内核性能基准测试
 * 用于展示和测量操作系统内核的各项性能指标
 */

#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/time.h>
#include <sys/wait.h>
#include <sys/mman.h>
#include <sched.h>

/* 获取当前时间(微秒) */
long get_time_us(void) {
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return tv.tv_sec * 1000000L + tv.tv_usec;
}

/* Print separator line */
void print_separator(void) {
    printf("===========================================================\n");
}

/* Print test header */
void print_test_header(const char *name) {
    printf("\n");
    print_separator();
    printf("  [TEST] %s\n", name);
    print_separator();
}

/* 打印结果 */
void print_result(const char *metric, long value, const char *unit) {
    printf("  %-30s %10ld %s\n", metric, value, unit);
}

void print_result_float(const char *metric, double value, const char *unit) {
    printf("  %-30s %10.2f %s\n", metric, value, unit);
}

/* ============================================================
 * 测试1: 系统调用开销
 * ============================================================ */
void bench_syscall(void) {
    print_test_header("System Call Overhead (getpid)");
    
    const int iterations = 100000;
    long start = get_time_us();
    
    for (int i = 0; i < iterations; i++) {
        getpid();
    }
    
    long elapsed = get_time_us() - start;
    double per_call = (double)elapsed / iterations;
    
    print_result("Total time", elapsed, "µs");
    print_result("Iterations", iterations, "calls");
    print_result_float("Time per syscall", per_call, "µs");
    print_result_float("Syscalls per second", 1000000.0 / per_call, "calls/s");
}

/* ============================================================
 * 测试2: 进程创建 (fork)
 * ============================================================ */
void bench_fork(void) {
    print_test_header("Process Creation (fork/exit)");
    
    const int iterations = 100;
    long start = get_time_us();
    
    for (int i = 0; i < iterations; i++) {
        pid_t pid = fork();
        if (pid == 0) {
            _exit(0);  /* 子进程立即退出 */
        } else if (pid > 0) {
            waitpid(pid, NULL, 0);
        }
    }
    
    long elapsed = get_time_us() - start;
    double per_fork = (double)elapsed / iterations;
    
    print_result("Total time", elapsed, "µs");
    print_result("Forks completed", iterations, "processes");
    print_result_float("Time per fork+exit+wait", per_fork, "µs");
    print_result_float("Forks per second", 1000000.0 / per_fork, "forks/s");
}

/* ============================================================
 * 测试3: 内存分配
 * ============================================================ */
void bench_memory(void) {
    print_test_header("Memory Allocation (mmap/munmap)");
    
    const int iterations = 1000;
    const size_t size = 4096;  /* 1页 */
    long start = get_time_us();
    
    for (int i = 0; i < iterations; i++) {
        void *ptr = mmap(NULL, size, PROT_READ | PROT_WRITE,
                        MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        if (ptr != MAP_FAILED) {
            /* 触发实际分配 */
            memset(ptr, 0, size);
            munmap(ptr, size);
        }
    }
    
    long elapsed = get_time_us() - start;
    double per_alloc = (double)elapsed / iterations;
    
    print_result("Total time", elapsed, "µs");
    print_result("Allocations", iterations, "pages");
    print_result("Page size", (long)size, "bytes");
    print_result_float("Time per mmap+munmap", per_alloc, "µs");
}

/* ============================================================
 * 测试4: 文件I/O
 * ============================================================ */
void bench_file_io(void) {
    print_test_header("File I/O (write/read)");
    
    const char *filename = "/tmp/bench_test.dat";
    const int iterations = 1000;
    const size_t block_size = 4096;
    char buffer[4096];
    
    /* 初始化buffer */
    memset(buffer, 'A', block_size);
    
    /* 写入测试 */
    int fd = open(filename, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd < 0) {
        printf("  [SKIP] Cannot create test file\n");
        return;
    }
    
    long start = get_time_us();
    for (int i = 0; i < iterations; i++) {
        write(fd, buffer, block_size);
    }
    fsync(fd);
    long write_time = get_time_us() - start;
    close(fd);
    
    /* 读取测试 */
    fd = open(filename, O_RDONLY);
    if (fd < 0) {
        printf("  [SKIP] Cannot open test file\n");
        unlink(filename);
        return;
    }
    
    start = get_time_us();
    for (int i = 0; i < iterations; i++) {
        read(fd, buffer, block_size);
    }
    long read_time = get_time_us() - start;
    close(fd);
    
    /* 清理 */
    unlink(filename);
    
    long total_bytes = (long)iterations * block_size;
    double write_mbps = (double)total_bytes / write_time;  /* MB/s */
    double read_mbps = (double)total_bytes / read_time;
    
    print_result("Block size", (long)block_size, "bytes");
    print_result("Total data", total_bytes / 1024, "KB");
    printf("  ─────────────────────────────────────────────────────────\n");
    print_result("Write time", write_time, "µs");
    print_result_float("Write throughput", write_mbps, "MB/s");
    printf("  ─────────────────────────────────────────────────────────\n");
    print_result("Read time", read_time, "µs");
    print_result_float("Read throughput", read_mbps, "MB/s");
}

/* ============================================================
 * 测试5: 管道通信
 * ============================================================ */
void bench_pipe(void) {
    print_test_header("Pipe Communication");
    
    int pipefd[2];
    if (pipe(pipefd) < 0) {
        printf("  [SKIP] pipe() failed\n");
        return;
    }
    
    const int iterations = 10000;
    const size_t msg_size = 64;
    char buffer[64];
    memset(buffer, 'X', msg_size);
    
    long start = get_time_us();
    
    pid_t pid = fork();
    if (pid == 0) {
        /* 子进程：读取 */
        close(pipefd[1]);
        for (int i = 0; i < iterations; i++) {
            read(pipefd[0], buffer, msg_size);
        }
        close(pipefd[0]);
        _exit(0);
    } else {
        /* 父进程：写入 */
        close(pipefd[0]);
        for (int i = 0; i < iterations; i++) {
            write(pipefd[1], buffer, msg_size);
        }
        close(pipefd[1]);
        waitpid(pid, NULL, 0);
    }
    
    long elapsed = get_time_us() - start;
    double per_msg = (double)elapsed / iterations;
    
    print_result("Message size", (long)msg_size, "bytes");
    print_result("Messages sent", iterations, "msgs");
    print_result("Total time", elapsed, "µs");
    print_result_float("Time per message", per_msg, "µs");
    print_result_float("Messages per second", 1000000.0 / per_msg, "msgs/s");
}

/* ============================================================
 * 测试6: 上下文切换
 * ============================================================ */
void bench_context_switch(void) {
    print_test_header("Context Switch (yield)");
    
    const int iterations = 10000;
    long start = get_time_us();
    
    for (int i = 0; i < iterations; i++) {
        sched_yield();
    }
    
    long elapsed = get_time_us() - start;
    double per_yield = (double)elapsed / iterations;
    
    print_result("Total time", elapsed, "µs");
    print_result("Yields", iterations, "times");
    print_result_float("Time per yield", per_yield, "µs");
}

/* ============================================================
 * 测试7: 时间获取
 * ============================================================ */
void bench_time(void) {
    print_test_header("Time Acquisition (gettimeofday)");
    
    const int iterations = 100000;
    struct timeval tv;
    
    long start = get_time_us();
    for (int i = 0; i < iterations; i++) {
        gettimeofday(&tv, NULL);
    }
    long elapsed = get_time_us() - start;
    
    double per_call = (double)elapsed / iterations;
    
    print_result("Total time", elapsed, "µs");
    print_result("Iterations", iterations, "calls");
    print_result_float("Time per call", per_call, "µs");
}

/* ============================================================
 * 主程序
 * ============================================================ */
int main(int argc, char *argv[]) {
    printf("\n");
    printf("+-----------------------------------------------------------+\n");
    printf("|          OS KERNEL BENCHMARK SUITE                        |\n");
    printf("|                                                           |\n");
    printf("|  Testing kernel performance metrics                       |\n");
    printf("+-----------------------------------------------------------+\n");
    
    int run_all = (argc < 2);
    
    for (int i = 1; i < argc || run_all; i++) {
        const char *test = run_all ? "all" : argv[i];
        
        if (run_all || strcmp(test, "all") == 0 || strcmp(test, "syscall") == 0) {
            bench_syscall();
            if (!run_all && strcmp(test, "syscall") == 0) continue;
        }
        
        if (run_all || strcmp(test, "all") == 0 || strcmp(test, "fork") == 0) {
            bench_fork();
            if (!run_all && strcmp(test, "fork") == 0) continue;
        }
        
        if (run_all || strcmp(test, "all") == 0 || strcmp(test, "memory") == 0) {
            bench_memory();
            if (!run_all && strcmp(test, "memory") == 0) continue;
        }
        
        if (run_all || strcmp(test, "all") == 0 || strcmp(test, "file") == 0) {
            bench_file_io();
            if (!run_all && strcmp(test, "file") == 0) continue;
        }
        
        if (run_all || strcmp(test, "all") == 0 || strcmp(test, "pipe") == 0) {
            bench_pipe();
            if (!run_all && strcmp(test, "pipe") == 0) continue;
        }
        
        if (run_all || strcmp(test, "all") == 0 || strcmp(test, "yield") == 0) {
            bench_context_switch();
            if (!run_all && strcmp(test, "yield") == 0) continue;
        }
        
        if (run_all || strcmp(test, "all") == 0 || strcmp(test, "time") == 0) {
            bench_time();
            if (!run_all && strcmp(test, "time") == 0) continue;
        }
        
        if (run_all) break;
    }
    
    printf("\n");
    print_separator();
    printf("  [OK] Benchmark completed!\n");
    
    return 0;
}
