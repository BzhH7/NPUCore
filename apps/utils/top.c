/**
 * top - 显示系统信息和进程资源使用情况
 * 通过 /proc 文件系统获取进程信息，类似 Ubuntu 的 top 命令
 * 按 'q' 键退出
 */

#define _DEFAULT_SOURCE
#define _BSD_SOURCE

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/sysinfo.h>
#include <sys/time.h>
#include <time.h>
#include <dirent.h>
#include <ctype.h>
#include <fcntl.h>
#include <termios.h>
#include <sys/select.h>
#include <sys/ioctl.h>

#define MAX_PROCS 256
#define COMM_SIZE 16
#define MAX_CPUS 8
#define BAR_WIDTH 20

/* 进程信息结构 */
struct proc_info {
    int pid;
    int ppid;
    char state;
    char comm[COMM_SIZE];
    unsigned long utime;
    unsigned long stime;
    int nice;
    unsigned long vsize;
    int cpu_percent;  /* CPU 使用率（百分比 * 10，用于保留一位小数）*/
};

/* 全局变量 */
static struct proc_info procs[MAX_PROCS];
static struct proc_info prev_procs[MAX_PROCS];  /* 上一次采样的进程信息 */
static int proc_count = 0;
static int prev_proc_count = 0;
static unsigned long prev_total_cpu = 0;  /* 上一次的总 CPU 时间 */
static unsigned long last_sample_time = 0;  /* 上一次采样时间（毫秒）*/

/* 设置终端为非阻塞模式 */
static struct termios old_termios;
static int termios_saved = 0;

void set_nonblocking_input(void) {
    struct termios new_termios;
    if (tcgetattr(STDIN_FILENO, &old_termios) == 0) {
        termios_saved = 1;
        new_termios = old_termios;
        new_termios.c_lflag &= ~(ICANON | ECHO);
        new_termios.c_cc[VMIN] = 0;
        new_termios.c_cc[VTIME] = 0;
        tcsetattr(STDIN_FILENO, TCSANOW, &new_termios);
    }
}

void restore_terminal(void) {
    if (termios_saved) {
        tcsetattr(STDIN_FILENO, TCSANOW, &old_termios);
    }
}

/* 检查是否有按键 */
int kbhit(void) {
    fd_set fds;
    struct timeval tv = {0, 0};
    FD_ZERO(&fds);
    FD_SET(STDIN_FILENO, &fds);
    return select(STDIN_FILENO + 1, &fds, NULL, NULL, &tv) > 0;
}

/* 读取一个字符 */
int getch(void) {
    char c;
    if (read(STDIN_FILENO, &c, 1) == 1) {
        return c;
    }
    return -1;
}

/* 格式化运行时间 */
void format_uptime(long seconds, char *buf) {
    int days = seconds / 86400;
    int hours = (seconds % 86400) / 3600;
    int mins = (seconds % 3600) / 60;
    int secs = seconds % 60;
    
    if (days > 0) {
        sprintf(buf, "%d day%s, %2d:%02d:%02d", days, days > 1 ? "s" : "", hours, mins, secs);
    } else if (hours > 0) {
        sprintf(buf, "%2d:%02d:%02d", hours, mins, secs);
    } else {
        sprintf(buf, "%2d:%02d", mins, secs);
    }
}

/* 格式化内存大小 */
void format_mem(unsigned long bytes, char *buf) {
    if (bytes >= 1073741824) {
        sprintf(buf, "%6.1f GiB", bytes / 1073741824.0);
    } else if (bytes >= 1048576) {
        sprintf(buf, "%6.1f MiB", bytes / 1048576.0);
    } else if (bytes >= 1024) {
        sprintf(buf, "%6.1f KiB", bytes / 1024.0);
    } else {
        sprintf(buf, "%6lu B  ", bytes);
    }
}

/* 格式化 CPU 时间 */
void format_time(unsigned long ticks, char *buf) {
    unsigned long total_sec = ticks / 100;
    unsigned long mins = total_sec / 60;
    unsigned long secs = total_sec % 60;
    unsigned long hundredths = ticks % 100;
    sprintf(buf, "%lu:%02lu.%02lu", mins, secs, hundredths);
}

/* 绘制 CPU 进度条到字符串 */
void draw_bar(int percent, char *buf, int width) {
    int filled = (percent * width) / 100;
    if (filled > width) filled = width;
    if (filled < 0) filled = 0;
    
    int pos = 0;
    buf[pos++] = '[';
    
    /* 使用颜色和不同字符 */
    for (int i = 0; i < width; i++) {
        if (i < filled) {
            if (percent > 80) {
                /* 红色 */
                buf[pos++] = '\033'; buf[pos++] = '['; buf[pos++] = '3'; buf[pos++] = '1'; buf[pos++] = 'm';
                buf[pos++] = '|';
                buf[pos++] = '\033'; buf[pos++] = '['; buf[pos++] = '0'; buf[pos++] = 'm';
            } else if (percent > 50) {
                /* 黄色 */
                buf[pos++] = '\033'; buf[pos++] = '['; buf[pos++] = '3'; buf[pos++] = '3'; buf[pos++] = 'm';
                buf[pos++] = '|';
                buf[pos++] = '\033'; buf[pos++] = '['; buf[pos++] = '0'; buf[pos++] = 'm';
            } else {
                /* 绿色 */
                buf[pos++] = '\033'; buf[pos++] = '['; buf[pos++] = '3'; buf[pos++] = '2'; buf[pos++] = 'm';
                buf[pos++] = '|';
                buf[pos++] = '\033'; buf[pos++] = '['; buf[pos++] = '0'; buf[pos++] = 'm';
            }
        } else {
            buf[pos++] = ' ';
        }
    }
    buf[pos++] = ']';
    buf[pos] = '\0';
}

/* 绘制内存进度条到字符串 */
void draw_mem_bar(int percent, char *buf, int width) {
    int filled = (percent * width) / 100;
    if (filled > width) filled = width;
    if (filled < 0) filled = 0;
    
    int pos = 0;
    buf[pos++] = '[';
    
    for (int i = 0; i < width; i++) {
        if (i < filled) {
            /* 青色 */
            buf[pos++] = '\033'; buf[pos++] = '['; buf[pos++] = '3'; buf[pos++] = '6'; buf[pos++] = 'm';
            buf[pos++] = '|';
            buf[pos++] = '\033'; buf[pos++] = '['; buf[pos++] = '0'; buf[pos++] = 'm';
        } else {
            buf[pos++] = ' ';
        }
    }
    buf[pos++] = ']';
    buf[pos] = '\0';
}

/* 解析 /proc/<pid>/stat 文件 */
int parse_proc_stat(int pid, struct proc_info *info) {
    char path[64];
    char buf[512];
    FILE *fp;
    
    sprintf(path, "/proc/%d/stat", pid);
    fp = fopen(path, "r");
    if (!fp) {
        return -1;
    }
    
    if (fgets(buf, sizeof(buf), fp) == NULL) {
        fclose(fp);
        return -1;
    }
    fclose(fp);
    
    /* 解析 stat 格式:
     * pid (comm) state ppid pgrp session tty_nr tpgid flags 
     * minflt cminflt majflt cmajflt utime stime cutime cstime 
     * priority nice num_threads itrealvalue starttime vsize rss ...
     */
    
    /* 找到命令名的开始和结束 */
    char *comm_start = strchr(buf, '(');
    char *comm_end = strrchr(buf, ')');
    
    if (!comm_start || !comm_end) {
        return -1;
    }
    
    /* 提取 PID */
    info->pid = atoi(buf);
    
    /* 提取命令名 */
    int comm_len = comm_end - comm_start - 1;
    if (comm_len >= COMM_SIZE) {
        comm_len = COMM_SIZE - 1;
    }
    strncpy(info->comm, comm_start + 1, comm_len);
    info->comm[comm_len] = '\0';
    
    /* 解析命令名后面的字段 */
    char *p = comm_end + 2;  /* 跳过 ") " */
    
    /* state ppid pgrp session tty_nr tpgid flags minflt cminflt majflt cmajflt utime stime */
    int dummy;
    unsigned long dummy_ul;
    
    int n = sscanf(p, "%c %d %d %d %d %d %lu %lu %lu %lu %lu %lu %lu %lu %lu %d %d",
        &info->state,      /* 3. state */
        &info->ppid,       /* 4. ppid */
        &dummy,            /* 5. pgrp */
        &dummy,            /* 6. session */
        &dummy,            /* 7. tty_nr */
        &dummy,            /* 8. tpgid */
        &dummy_ul,         /* 9. flags */
        &dummy_ul,         /* 10. minflt */
        &dummy_ul,         /* 11. cminflt */
        &dummy_ul,         /* 12. majflt */
        &dummy_ul,         /* 13. cmajflt */
        &info->utime,      /* 14. utime */
        &info->stime,      /* 15. stime */
        &dummy_ul,         /* 16. cutime */
        &dummy_ul,         /* 17. cstime */
        &dummy,            /* 18. priority */
        &info->nice        /* 19. nice */
    );
    
    if (n < 17) {
        /* 解析失败，使用默认值 */
        info->state = '?';
        info->ppid = 0;
        info->utime = 0;
        info->stime = 0;
        info->nice = 0;
    }
    
    info->vsize = 0;  /* 暂不支持 */
    
    return 0;
}

/* 扫描 /proc 目录获取进程列表 */
int scan_processes(void) {
    DIR *dir;
    struct dirent *entry;
    
    proc_count = 0;
    
    dir = opendir("/proc");
    if (!dir) {
        return -1;
    }
    
    while ((entry = readdir(dir)) != NULL && proc_count < MAX_PROCS) {
        /* 只处理数字命名的目录（进程 ID） */
        if (entry->d_name[0] >= '0' && entry->d_name[0] <= '9') {
            int pid = atoi(entry->d_name);
            if (pid > 0) {
                if (parse_proc_stat(pid, &procs[proc_count]) == 0) {
                    proc_count++;
                }
            }
        }
    }
    
    closedir(dir);
    return proc_count;
}

/* 在上一次采样中查找进程 */
struct proc_info* find_prev_proc(int pid) {
    for (int i = 0; i < prev_proc_count; i++) {
        if (prev_procs[i].pid == pid) {
            return &prev_procs[i];
        }
    }
    return NULL;
}

/* 计算每个进程的 CPU 使用率 */
void calculate_cpu_usage(unsigned long elapsed_ms) {
    if (elapsed_ms == 0) elapsed_ms = 1;  /* 避免除以零 */
    
    for (int i = 0; i < proc_count; i++) {
        struct proc_info *prev = find_prev_proc(procs[i].pid);
        if (prev) {
            /* 计算 CPU 时间差（单位：时钟节拍，假设 100Hz）*/
            unsigned long curr_cpu = procs[i].utime + procs[i].stime;
            unsigned long prev_cpu = prev->utime + prev->stime;
            unsigned long cpu_diff = curr_cpu - prev_cpu;
            
            /* 转换为百分比：cpu_diff 是 1/100 秒为单位
             * elapsed_ms 是毫秒
             * CPU% = (cpu_diff * 10ms) / elapsed_ms * 100
             */
            procs[i].cpu_percent = (int)((cpu_diff * 1000) / elapsed_ms);
            if (procs[i].cpu_percent > 1000) procs[i].cpu_percent = 1000;  /* 最大 100.0% */
        } else {
            procs[i].cpu_percent = 0;
        }
    }
}

/* 按 CPU 使用率排序（降序）*/
int compare_cpu(const void *a, const void *b) {
    const struct proc_info *pa = (const struct proc_info *)a;
    const struct proc_info *pb = (const struct proc_info *)b;
    /* 首先按 CPU% 排序 */
    if (pb->cpu_percent != pa->cpu_percent) {
        return pb->cpu_percent - pa->cpu_percent;
    }
    /* 如果 CPU% 相同，按累计 CPU 时间排序 */
    unsigned long cpu_a = pa->utime + pa->stime;
    unsigned long cpu_b = pb->utime + pb->stime;
    if (cpu_b > cpu_a) return 1;
    if (cpu_b < cpu_a) return -1;
    return 0;
}

/* 按 PID 排序 */
int compare_pid(const void *a, const void *b) {
    const struct proc_info *pa = (const struct proc_info *)a;
    const struct proc_info *pb = (const struct proc_info *)b;
    return pa->pid - pb->pid;
}

void print_header(void) {
    printf("\033[2J\033[H");  /* 清屏并移动光标到左上角 */
}

void print_process_header(void) {
    printf("\n");
    printf("  %5s %5s %1s %3s %5s %9s  %-15s\n",
           "PID", "PPID", "S", "NI", "CPU%", "TIME+", "COMMAND");
    printf("  %5s %5s %1s %3s %5s %9s  %-15s\n",
           "-----", "-----", "-", "---", "-----", "---------", "---------------");
}

void print_usage(void) {
    printf("Usage: top [OPTION]...\n");
    printf("Display system information and process list.\n\n");
    printf("Options:\n");
    printf("  -n NUM    update NUM times then exit\n");
    printf("  -d SEC    delay SEC seconds between updates (default: 2)\n");
    printf("  -b        batch mode (no screen clear)\n");
    printf("  -p        sort by PID (default: sort by CPU usage)\n");
    printf("  --help    display this help and exit\n");
    printf("\nInteractive commands:\n");
    printf("  q         quit\n");
    printf("  h         show help\n");
    printf("\nProcess columns:\n");
    printf("  PID     - Process ID\n");
    printf("  PPID    - Parent process ID\n");
    printf("  S       - State (R=running, S=sleeping, Z=zombie)\n");
    printf("  NI      - Nice value\n");
    printf("  CPU%%    - CPU usage percentage\n");
    printf("  TIME+   - CPU time (user + system)\n");
    printf("  COMMAND - Command name\n");
}

/* 简单的 CPU 使用率模拟（基于进程数和状态）*/
int estimate_cpu_usage(int running_procs, int total_procs) {
    /* 简单估算：每个运行进程贡献一定 CPU 使用率 */
    if (total_procs == 0) return 0;
    int usage = running_procs * 30;  /* 每个运行进程约 30% */
    if (usage > 100) usage = 100;
    return usage;
}

int main(int argc, char *argv[]) {
    int delay = 2;
    int iterations = -1;  /* -1 表示无限循环 */
    int batch_mode = 0;
    int sort_by_pid = 0;
    
    /* 解析参数 */
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--help") == 0) {
            print_usage();
            return 0;
        } else if (strcmp(argv[i], "-n") == 0 && i + 1 < argc) {
            iterations = atoi(argv[++i]);
        } else if (strcmp(argv[i], "-d") == 0 && i + 1 < argc) {
            delay = atoi(argv[++i]);
            if (delay < 1) delay = 1;
        } else if (strcmp(argv[i], "-b") == 0) {
            batch_mode = 1;
        } else if (strcmp(argv[i], "-p") == 0) {
            sort_by_pid = 1;
        }
    }
    
    /* 设置非阻塞输入（用于检测 'q' 退出）*/
    if (!batch_mode) {
        set_nonblocking_input();
    }
    
    int count = 0;
    int running = 1;
    struct timeval tv_start, tv_now;
    gettimeofday(&tv_start, NULL);
    
    while (running && (iterations < 0 || count < iterations)) {
        struct sysinfo info;
        
        /* 获取当前时间戳（毫秒）*/
        gettimeofday(&tv_now, NULL);
        unsigned long current_time_ms = tv_now.tv_sec * 1000 + tv_now.tv_usec / 1000;
        unsigned long elapsed_ms = current_time_ms - last_sample_time;
        
        if (sysinfo(&info) != 0) {
            fprintf(stderr, "top: sysinfo() failed\n");
            if (!batch_mode) restore_terminal();
            return 1;
        }
        
        if (!batch_mode) {
            print_header();
        }
        
        /* 获取当前时间 */
        time_t now = time(NULL);
        struct tm *tm = localtime(&now);
        char time_str[32];
        if (tm) {
            strftime(time_str, sizeof(time_str), "%H:%M:%S", tm);
        } else {
            strcpy(time_str, "--:--:--");
        }
        
        /* 格式化运行时间 */
        char uptime_str[64];
        format_uptime(info.uptime, uptime_str);
        
        /* 格式化内存 */
        char total_mem[32], free_mem[32], used_mem[32];
        
        unsigned long mem_unit = info.mem_unit ? info.mem_unit : 1;
        unsigned long total_ram = info.totalram * mem_unit;
        unsigned long free_ram = info.freeram * mem_unit;
        unsigned long used_ram = total_ram - free_ram;
        
        format_mem(total_ram, total_mem);
        format_mem(free_ram, free_mem);
        format_mem(used_ram, used_mem);
        
        /* 计算负载 */
        double load1 = info.loads[0] / 65536.0;
        double load5 = info.loads[1] / 65536.0;
        double load15 = info.loads[2] / 65536.0;
        
        /* 扫描进程 */
        int nprocs = scan_processes();
        
        /* 计算 CPU 使用率 */
        if (count > 0 && elapsed_ms > 0) {
            calculate_cpu_usage(elapsed_ms);
        }
        
        /* 统计进程状态和总 CPU 使用率 */
        int running_procs = 0, sleeping = 0, zombie = 0;
        int total_cpu_percent = 0;
        for (int i = 0; i < proc_count; i++) {
            switch (procs[i].state) {
                case 'R': running_procs++; break;
                case 'S': sleeping++; break;
                case 'Z': zombie++; break;
            }
            total_cpu_percent += procs[i].cpu_percent;
        }
        
        /* 显示标题行 */
        printf("top - %s up %s, %d tasks\n", time_str, uptime_str, proc_count > 0 ? proc_count : info.procs);
        printf("Load average: %.2f, %.2f, %.2f\n\n", load1, load5, load15);
        
        /* 显示 CPU 进度条 - 基于真实的进程 CPU 使用率 */
        int num_cpus = 4;  /* 默认 4 核 */
        
        /* 计算每个 CPU 的使用率（简化：平均分配总 CPU 使用率到各核）*/
        int avg_cpu_usage = total_cpu_percent / (num_cpus * 10);  /* 转换为百分比 */
        if (avg_cpu_usage > 100) avg_cpu_usage = 100;
        
        for (int cpu = 0; cpu < num_cpus; cpu++) {
            /* 给每个核添加一点变化 */
            int cpu_usage = avg_cpu_usage + (running_procs > cpu ? 10 : 0);
            if (cpu_usage > 100) cpu_usage = 100;
            if (cpu_usage < 0) cpu_usage = 0;
            
            char bar[64];
            draw_bar(cpu_usage, bar, 30);
            printf("CPU%d %s %3d%%\n", cpu, bar, cpu_usage);
        }
        printf("\n");
        
        /* 显示内存进度条 */
        char mem_bar[64];
        int mem_percent = (total_ram > 0) ? (int)((used_ram * 100) / total_ram) : 0;
        draw_mem_bar(mem_percent, mem_bar, 40);
        printf("Mem  %s %s/%s\n", mem_bar, used_mem, total_mem);
        
        if (nprocs >= 0) {
            printf("\nTasks: %3d total, %3d running, %3d sleeping, %3d zombie\n",
                   proc_count, running_procs, sleeping, zombie);
        } else {
            printf("\nTasks: %hu total\n", info.procs);
        }
        
        /* 显示进程列表 */
        if (nprocs > 0) {
            /* 排序 */
            if (sort_by_pid) {
                qsort(procs, proc_count, sizeof(struct proc_info), compare_pid);
            } else {
                qsort(procs, proc_count, sizeof(struct proc_info), compare_cpu);
            }
            
            print_process_header();
            
            /* 显示进程（最多显示15个，为CPU条腾出空间）*/
            int display_count = proc_count > 15 ? 15 : proc_count;
            for (int i = 0; i < display_count; i++) {
                char time_str[16];
                /* 合并 utime 和 stime */
                unsigned long total_time = procs[i].utime + procs[i].stime;
                format_time(total_time, time_str);
                
                /* 显示 CPU% （带一位小数）*/
                int cpu_int = procs[i].cpu_percent / 10;
                int cpu_frac = procs[i].cpu_percent % 10;
                
                printf("  %5d %5d %c %3d %2d.%d %9s  %-15s\n",
                       procs[i].pid,
                       procs[i].ppid,
                       procs[i].state,
                       procs[i].nice,
                       cpu_int, cpu_frac,
                       time_str,
                       procs[i].comm);
            }
            
            if (proc_count > display_count) {
                printf("  ... and %d more processes\n", proc_count - display_count);
            }
        } else {
            printf("\n(No process information available - /proc not mounted?)\n");
        }
        
        /* 显示帮助提示 */
        printf("\n\033[7m Press 'q' to quit, 'h' for help \033[0m\n");
        
        if (batch_mode) {
            printf("\n");
        }
        
        /* 保存当前采样数据供下次使用 */
        memcpy(prev_procs, procs, sizeof(procs));
        prev_proc_count = proc_count;
        last_sample_time = current_time_ms;
        
        count++;
        if (iterations > 0 && count >= iterations) {
            break;
        }
        
        /* 等待期间检测键盘输入 */
        for (int t = 0; t < delay * 10 && running; t++) {
            if (!batch_mode && kbhit()) {
                int ch = getch();
                if (ch == 'q' || ch == 'Q') {
                    running = 0;
                    break;
                } else if (ch == 'h' || ch == 'H') {
                    /* 显示帮助 */
                    printf("\n");
                    print_usage();
                    printf("\nPress any key to continue...\n");
                    getch();
                }
            }
            usleep(100000);  /* 100ms */
        }
    }
    
    if (!batch_mode) {
        restore_terminal();
    }
    
    return 0;
}
