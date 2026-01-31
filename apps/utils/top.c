/**
 * top - 显示系统信息和资源使用情况
 * 注意：由于内核没有实现 /proc 文件系统，只能显示基本系统信息
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/sysinfo.h>
#include <time.h>

/* 格式化运行时间 */
void format_uptime(long seconds, char *buf) {
    int days = seconds / 86400;
    int hours = (seconds % 86400) / 3600;
    int mins = (seconds % 3600) / 60;
    
    if (days > 0) {
        sprintf(buf, "%d days, %2d:%02d", days, hours, mins);
    } else {
        sprintf(buf, "%2d:%02d", hours, mins);
    }
}

/* 格式化内存大小 */
void format_mem(unsigned long bytes, char *buf) {
    if (bytes >= 1073741824) {
        sprintf(buf, "%.1f GB", bytes / 1073741824.0);
    } else if (bytes >= 1048576) {
        sprintf(buf, "%.1f MB", bytes / 1048576.0);
    } else if (bytes >= 1024) {
        sprintf(buf, "%.1f KB", bytes / 1024.0);
    } else {
        sprintf(buf, "%lu B", bytes);
    }
}

void print_header(void) {
    printf("\033[2J\033[H");  /* 清屏并移动光标到左上角 */
    printf("========================================\n");
    printf("        System Information (top)        \n");
    printf("========================================\n\n");
}

void print_usage(void) {
    printf("Usage: top [OPTION]...\n");
    printf("Display system information.\n\n");
    printf("Options:\n");
    printf("  -n NUM    update NUM times then exit\n");
    printf("  -d SEC    delay SEC seconds between updates (default: 2)\n");
    printf("  -b        batch mode (no screen clear)\n");
    printf("  --help    display this help and exit\n");
    printf("\nNote: This is a simplified version. Full process listing\n");
    printf("      is not available without /proc filesystem support.\n");
}

int main(int argc, char *argv[]) {
    int delay = 2;
    int iterations = -1;  /* -1 表示无限循环 */
    int batch_mode = 0;
    
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
        }
    }
    
    int count = 0;
    
    while (iterations < 0 || count < iterations) {
        struct sysinfo info;
        
        if (sysinfo(&info) != 0) {
            fprintf(stderr, "top: sysinfo() failed\n");
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
        char total_swap[32], free_swap[32], used_swap[32];
        
        unsigned long mem_unit = info.mem_unit ? info.mem_unit : 1;
        unsigned long total_ram = info.totalram * mem_unit;
        unsigned long free_ram = info.freeram * mem_unit;
        unsigned long used_ram = total_ram - free_ram;
        unsigned long total_swp = info.totalswap * mem_unit;
        unsigned long free_swp = info.freeswap * mem_unit;
        unsigned long used_swp = total_swp - free_swp;
        
        format_mem(total_ram, total_mem);
        format_mem(free_ram, free_mem);
        format_mem(used_ram, used_mem);
        format_mem(total_swp, total_swap);
        format_mem(free_swp, free_swap);
        format_mem(used_swp, used_swap);
        
        /* 计算负载 (load average 是 16 位定点数，除以 65536) */
        double load1 = info.loads[0] / 65536.0;
        double load5 = info.loads[1] / 65536.0;
        double load15 = info.loads[2] / 65536.0;
        
        /* 显示系统信息 */
        printf("top - %s up %s, %d users\n", time_str, uptime_str, 1);
        printf("Load average: %.2f, %.2f, %.2f\n", load1, load5, load15);
        
        /* 获取 CPU 核心数 */
        long nprocs = sysconf(_SC_NPROCESSORS_ONLN);
        if (nprocs > 0) {
            printf("CPU cores: %ld\n\n", nprocs);
        } else {
            printf("\n");
        }
        
        printf("Tasks: %hu total\n\n", info.procs);
        
        printf("Memory:\n");
        printf("  Total:  %s\n", total_mem);
        printf("  Used:   %s\n", used_mem);
        printf("  Free:   %s\n", free_mem);
        printf("  Shared: ");
        char shared_mem[32];
        format_mem(info.sharedram * mem_unit, shared_mem);
        printf("%s\n", shared_mem);
        printf("  Buffer: ");
        char buffer_mem[32];
        format_mem(info.bufferram * mem_unit, buffer_mem);
        printf("%s\n\n", buffer_mem);
        
        if (total_swp > 0) {
            printf("Swap:\n");
            printf("  Total: %s\n", total_swap);
            printf("  Used:  %s\n", used_swap);
            printf("  Free:  %s\n\n", free_swap);
        }
        
        printf("----------------------------------------\n");
        printf("Note: Process list not available\n");
        printf("      (/proc filesystem not implemented)\n");
        printf("----------------------------------------\n");
        
        if (batch_mode) {
            printf("\n");
        }
        
        count++;
        if (iterations > 0 && count >= iterations) {
            break;
        }
        
        sleep(delay);
    }
    
    return 0;
}
