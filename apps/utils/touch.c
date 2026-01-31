/**
 * touch - 创建空文件或更新文件时间戳
 */

#define _DEFAULT_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <errno.h>
#include <time.h>

static int opt_no_create = 0;  /* -c */

/* 创建或更新文件 */
int touch_file(const char *path) {
    struct stat st;
    
    /* 检查文件是否存在 */
    if (stat(path, &st) == 0) {
        /* 文件存在，更新时间戳 */
        /* 使用 utimensat 或 utime，这里用 open/close 触发 atime/mtime 更新 */
        int fd = open(path, O_RDWR);
        if (fd < 0) {
            /* 只读文件，尝试只打开读取来更新 atime */
            fd = open(path, O_RDONLY);
            if (fd < 0) {
                fprintf(stderr, "touch: cannot touch '%s': %s\n", path, strerror(errno));
                return 1;
            }
        }
        close(fd);
        return 0;
    }
    
    /* 文件不存在 */
    if (opt_no_create) {
        return 0;  /* -c 选项，不创建新文件 */
    }
    
    /* 创建新文件 */
    int fd = open(path, O_CREAT | O_WRONLY, 0644);
    if (fd < 0) {
        fprintf(stderr, "touch: cannot touch '%s': %s\n", path, strerror(errno));
        return 1;
    }
    close(fd);
    
    return 0;
}

void print_usage(void) {
    printf("Usage: touch [OPTION]... FILE...\n");
    printf("Update the access and modification times of each FILE to the current time.\n");
    printf("A FILE argument that does not exist is created empty.\n\n");
    printf("Options:\n");
    printf("  -c        do not create any files\n");
    printf("  --help    display this help and exit\n");
}

int main(int argc, char *argv[]) {
    int i;
    int ret = 0;
    int has_file = 0;
    
    if (argc < 2) {
        fprintf(stderr, "touch: missing file operand\n");
        fprintf(stderr, "Try 'touch --help' for more information.\n");
        return 1;
    }
    
    /* 解析选项 */
    for (i = 1; i < argc; i++) {
        if (argv[i][0] == '-' && argv[i][1] != '\0') {
            if (strcmp(argv[i], "--help") == 0) {
                print_usage();
                return 0;
            } else if (strcmp(argv[i], "-c") == 0) {
                opt_no_create = 1;
            } else {
                fprintf(stderr, "touch: invalid option -- '%s'\n", argv[i]);
                return 1;
            }
        } else {
            has_file = 1;
        }
    }
    
    if (!has_file) {
        fprintf(stderr, "touch: missing file operand\n");
        return 1;
    }
    
    /* 处理文件 */
    for (i = 1; i < argc; i++) {
        if (argv[i][0] == '-' && argv[i][1] != '\0') continue;
        ret |= touch_file(argv[i]);
    }
    
    return ret;
}
