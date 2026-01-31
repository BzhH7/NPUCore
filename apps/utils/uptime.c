/**
 * uptime - 显示系统运行时间和负载
 * 用于演示操作系统内核的时间和系统信息获取能力
 */

#include <stdio.h>
#include <sys/time.h>
#include <time.h>

/* 简单实现，假设启动时间为0 */
static struct timeval boot_time = {0, 0};
static int boot_time_set = 0;

int main(void) {
    struct timeval now;
    gettimeofday(&now, NULL);
    
    /* 第一次运行时记录启动时间 */
    if (!boot_time_set) {
        boot_time = now;
        boot_time_set = 1;
    }
    
    /* 计算运行时间 */
    long uptime_sec = now.tv_sec;  /* 假设内核启动时间为0 */
    
    int days = uptime_sec / 86400;
    int hours = (uptime_sec % 86400) / 3600;
    int minutes = (uptime_sec % 3600) / 60;
    int seconds = uptime_sec % 60;
    
    /* 获取当前时间 */
    time_t t = now.tv_sec;
    struct tm *tm_info = localtime(&t);
    
    printf(" %02d:%02d:%02d up ", 
           tm_info->tm_hour, tm_info->tm_min, tm_info->tm_sec);
    
    if (days > 0) {
        printf("%d day%s, ", days, days > 1 ? "s" : "");
    }
    
    if (hours > 0) {
        printf("%d:%02d", hours, minutes);
    } else {
        printf("%d min", minutes);
    }
    
    printf(", load average: 0.00, 0.00, 0.00\n");
    
    return 0;
}
