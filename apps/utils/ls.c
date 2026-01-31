/**
 * ls - 列出目录内容
 * 支持 -l (长格式) 和 -a (显示隐藏文件) 选项
 */

#define _DEFAULT_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <dirent.h>
#include <sys/stat.h>
#include <time.h>
#include <pwd.h>
#include <grp.h>

#define MAX_ENTRIES 4096

/* 选项标志 */
static int opt_long = 0;    /* -l */
static int opt_all = 0;     /* -a */
static int opt_human = 0;   /* -h */

/* 比较函数用于排序 */
int compare_names(const void *a, const void *b) {
    return strcmp(*(const char **)a, *(const char **)b);
}

/* 格式化文件权限 */
void format_mode(mode_t mode, char *buf) {
    buf[0] = S_ISDIR(mode) ? 'd' : (S_ISLNK(mode) ? 'l' : (S_ISCHR(mode) ? 'c' : (S_ISBLK(mode) ? 'b' : '-')));
    buf[1] = (mode & S_IRUSR) ? 'r' : '-';
    buf[2] = (mode & S_IWUSR) ? 'w' : '-';
    buf[3] = (mode & S_IXUSR) ? 'x' : '-';
    buf[4] = (mode & S_IRGRP) ? 'r' : '-';
    buf[5] = (mode & S_IWGRP) ? 'w' : '-';
    buf[6] = (mode & S_IXGRP) ? 'x' : '-';
    buf[7] = (mode & S_IROTH) ? 'r' : '-';
    buf[8] = (mode & S_IWOTH) ? 'w' : '-';
    buf[9] = (mode & S_IXOTH) ? 'x' : '-';
    buf[10] = '\0';
}

/* 格式化文件大小 (人类可读) */
void format_size(off_t size, char *buf) {
    if (!opt_human) {
        sprintf(buf, "%8ld", (long)size);
        return;
    }
    
    const char *units[] = {"", "K", "M", "G", "T"};
    int unit = 0;
    double sz = size;
    
    while (sz >= 1024 && unit < 4) {
        sz /= 1024;
        unit++;
    }
    
    if (unit == 0) {
        sprintf(buf, "%8ld", (long)size);
    } else {
        sprintf(buf, "%7.1f%s", sz, units[unit]);
    }
}

/* 格式化时间 */
void format_time(time_t t, char *buf) {
    struct tm *tm = localtime(&t);
    if (tm) {
        strftime(buf, 20, "%b %d %H:%M", tm);
    } else {
        strcpy(buf, "??? ?? ??:??");
    }
}

/* 打印单个文件信息 */
void print_entry(const char *path, const char *name, struct stat *st) {
    if (opt_long) {
        char mode_str[12];
        char size_str[16];
        char time_str[20];
        
        format_mode(st->st_mode, mode_str);
        format_size(st->st_size, size_str);
        format_time(st->st_mtime, time_str);
        
        /* 颜色输出 */
        if (S_ISDIR(st->st_mode)) {
            printf("%s %3ld %s %s \033[34m%s\033[0m\n",
                   mode_str, (long)st->st_nlink, size_str, time_str, name);
        } else if (st->st_mode & (S_IXUSR | S_IXGRP | S_IXOTH)) {
            printf("%s %3ld %s %s \033[32m%s\033[0m\n",
                   mode_str, (long)st->st_nlink, size_str, time_str, name);
        } else {
            printf("%s %3ld %s %s %s\n",
                   mode_str, (long)st->st_nlink, size_str, time_str, name);
        }
    } else {
        /* 简单输出 */
        if (S_ISDIR(st->st_mode)) {
            printf("\033[34m%s\033[0m  ", name);
        } else if (st->st_mode & (S_IXUSR | S_IXGRP | S_IXOTH)) {
            printf("\033[32m%s\033[0m  ", name);
        } else {
            printf("%s  ", name);
        }
    }
}

/* 列出目录内容 */
int list_dir(const char *path) {
    DIR *dir = opendir(path);
    if (!dir) {
        fprintf(stderr, "ls: cannot access '%s': No such file or directory\n", path);
        return 1;
    }
    
    /* 收集目录项 */
    char *entries[MAX_ENTRIES];
    int entry_count = 0;
    struct dirent *ent;
    
    while ((ent = readdir(dir)) != NULL && entry_count < MAX_ENTRIES) {
        /* 跳过隐藏文件 (除非 -a) */
        if (!opt_all && ent->d_name[0] == '.') {
            continue;
        }
        entries[entry_count] = strdup(ent->d_name);
        entry_count++;
    }
    closedir(dir);
    
    /* 排序 */
    qsort(entries, entry_count, sizeof(char *), compare_names);
    
    /* 打印每个条目 */
    for (int i = 0; i < entry_count; i++) {
        char full_path[1024];
        if (strcmp(path, ".") == 0) {
            snprintf(full_path, sizeof(full_path), "%s", entries[i]);
        } else {
            snprintf(full_path, sizeof(full_path), "%s/%s", path, entries[i]);
        }
        
        struct stat st;
        if (stat(full_path, &st) != 0) {
            /* 如果 stat 失败，尝试 lstat */
            if (lstat(full_path, &st) != 0) {
                memset(&st, 0, sizeof(st));
            }
        }
        
        print_entry(full_path, entries[i], &st);
        free(entries[i]);
    }
    
    if (!opt_long && entry_count > 0) {
        printf("\n");
    }
    
    return 0;
}

/* 列出单个文件 */
int list_file(const char *path) {
    struct stat st;
    if (stat(path, &st) != 0) {
        fprintf(stderr, "ls: cannot access '%s': No such file or directory\n", path);
        return 1;
    }
    
    /* 获取文件名部分 */
    const char *name = strrchr(path, '/');
    name = name ? name + 1 : path;
    
    print_entry(path, name, &st);
    if (!opt_long) {
        printf("\n");
    }
    
    return 0;
}

void print_usage(void) {
    printf("Usage: ls [OPTION]... [FILE]...\n");
    printf("List directory contents.\n\n");
    printf("Options:\n");
    printf("  -a        do not ignore entries starting with .\n");
    printf("  -l        use a long listing format\n");
    printf("  -h        with -l, print human readable sizes\n");
    printf("  --help    display this help and exit\n");
}

int main(int argc, char *argv[]) {
    int i;
    int first_path = 0;
    
    /* 解析选项 */
    for (i = 1; i < argc; i++) {
        if (argv[i][0] == '-') {
            if (strcmp(argv[i], "--help") == 0) {
                print_usage();
                return 0;
            }
            for (int j = 1; argv[i][j]; j++) {
                switch (argv[i][j]) {
                    case 'l': opt_long = 1; break;
                    case 'a': opt_all = 1; break;
                    case 'h': opt_human = 1; break;
                    default:
                        fprintf(stderr, "ls: invalid option -- '%c'\n", argv[i][j]);
                        return 1;
                }
            }
        } else {
            if (first_path == 0) first_path = i;
        }
    }
    
    /* 如果没有指定路径，列出当前目录 */
    if (first_path == 0) {
        return list_dir(".");
    }
    
    /* 列出指定的路径 */
    int ret = 0;
    int path_count = 0;
    
    /* 计算路径数量 */
    for (i = first_path; i < argc; i++) {
        if (argv[i][0] != '-') path_count++;
    }
    
    for (i = first_path; i < argc; i++) {
        if (argv[i][0] == '-') continue;
        
        struct stat st;
        if (stat(argv[i], &st) != 0) {
            fprintf(stderr, "ls: cannot access '%s': No such file or directory\n", argv[i]);
            ret = 1;
            continue;
        }
        
        if (path_count > 1) {
            printf("%s:\n", argv[i]);
        }
        
        if (S_ISDIR(st.st_mode)) {
            ret |= list_dir(argv[i]);
        } else {
            ret |= list_file(argv[i]);
        }
        
        if (path_count > 1 && i < argc - 1) {
            printf("\n");
        }
    }
    
    return ret;
}
