/**
 * cal - 显示日历
 * 用于演示操作系统内核的时间获取能力
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <sys/time.h>

const char *month_names[] = {
    "January", "February", "March", "April", "May", "June",
    "July", "August", "September", "October", "November", "December"
};

const char *day_names = "Su Mo Tu We Th Fr Sa";

/* 判断是否闰年 */
int is_leap_year(int year) {
    return (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
}

/* 获取某月的天数 */
int days_in_month(int year, int month) {
    int days[] = {31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31};
    if (month == 2 && is_leap_year(year)) {
        return 29;
    }
    return days[month - 1];
}

/* 计算某年某月1日是星期几 (0=Sunday) */
/* 使用Zeller公式 */
int day_of_week(int year, int month, int day) {
    if (month < 3) {
        month += 12;
        year--;
    }
    int q = day;
    int m = month;
    int k = year % 100;
    int j = year / 100;
    
    int h = (q + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 - 2 * j) % 7;
    return ((h + 6) % 7);  /* 转换为 0=Sunday */
}

/* 打印某月的日历 */
void print_month(int year, int month, int today_day) {
    int days = days_in_month(year, month);
    int start_day = day_of_week(year, month, 1);
    
    /* 打印月份和年份标题 */
    char title[30];
    snprintf(title, sizeof(title), "%s %d", month_names[month - 1], year);
    int padding = (20 - strlen(title)) / 2;
    printf("%*s%s\n", padding, "", title);
    
    /* 打印星期标题 */
    printf("%s\n", day_names);
    
    /* 打印日期 */
    int day = 1;
    for (int week = 0; week < 6 && day <= days; week++) {
        for (int dow = 0; dow < 7; dow++) {
            if (week == 0 && dow < start_day) {
                printf("   ");
            } else if (day <= days) {
                if (day == today_day) {
                    /* 高亮今天 */
                    printf("\033[7m%2d\033[0m ", day);
                } else {
                    printf("%2d ", day);
                }
                day++;
            }
        }
        printf("\n");
    }
}

/* 打印整年日历 */
void print_year(int year) {
    printf("\n");
    char title[10];
    snprintf(title, sizeof(title), "%d", year);
    int padding = (64 - strlen(title)) / 2;
    printf("%*s%s\n\n", padding, "", title);
    
    /* 每行打印3个月 */
    for (int row = 0; row < 4; row++) {
        int months[3] = {row * 3 + 1, row * 3 + 2, row * 3 + 3};
        
        /* 打印月份标题 */
        for (int i = 0; i < 3; i++) {
            char mtitle[20];
            snprintf(mtitle, sizeof(mtitle), "%s", month_names[months[i] - 1]);
            int pad = (20 - strlen(mtitle)) / 2;
            printf("%*s%-*s  ", pad, "", 20 - pad, mtitle);
        }
        printf("\n");
        
        /* 打印星期标题 */
        for (int i = 0; i < 3; i++) {
            printf("%s  ", day_names);
        }
        printf("\n");
        
        /* 打印日期行 */
        int days[3], starts[3], day_counters[3];
        for (int i = 0; i < 3; i++) {
            days[i] = days_in_month(year, months[i]);
            starts[i] = day_of_week(year, months[i], 1);
            day_counters[i] = 1;
        }
        
        for (int week = 0; week < 6; week++) {
            for (int m = 0; m < 3; m++) {
                for (int dow = 0; dow < 7; dow++) {
                    if (week == 0 && dow < starts[m]) {
                        printf("   ");
                    } else if (day_counters[m] <= days[m]) {
                        printf("%2d ", day_counters[m]);
                        day_counters[m]++;
                    } else {
                        printf("   ");
                    }
                }
                printf(" ");
            }
            printf("\n");
        }
        printf("\n");
    }
}

int main(int argc, char *argv[]) {
    struct timeval tv;
    struct tm *tm_info;
    
    gettimeofday(&tv, NULL);
    time_t now = tv.tv_sec;
    tm_info = localtime(&now);
    
    int year = tm_info->tm_year + 1900;
    int month = tm_info->tm_mon + 1;
    int today = tm_info->tm_mday;
    
    if (argc == 1) {
        /* 无参数：显示当前月份 */
        print_month(year, month, today);
    } else if (argc == 2) {
        /* 一个参数：年份，显示整年 */
        year = atoi(argv[1]);
        if (year < 1 || year > 9999) {
            fprintf(stderr, "cal: invalid year %s\n", argv[1]);
            return 1;
        }
        print_year(year);
    } else if (argc == 3) {
        /* 两个参数：月份和年份 */
        month = atoi(argv[1]);
        year = atoi(argv[2]);
        if (month < 1 || month > 12) {
            fprintf(stderr, "cal: invalid month %s\n", argv[1]);
            return 1;
        }
        if (year < 1 || year > 9999) {
            fprintf(stderr, "cal: invalid year %s\n", argv[2]);
            return 1;
        }
        print_month(year, month, -1);
    } else {
        fprintf(stderr, "Usage: cal [[month] year]\n");
        return 1;
    }
    
    return 0;
}
