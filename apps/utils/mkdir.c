/**
 * mkdir - 创建目录
 * 支持 -p (递归创建) 选项
 */

#define _DEFAULT_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <errno.h>

static int opt_parents = 0;  /* -p */
static int dir_mode = 0755;

/* 递归创建目录 */
int mkdir_p(const char *path) {
    char *tmp = strdup(path);
    char *p = tmp;
    int ret = 0;
    
    /* 跳过开头的 / */
    if (*p == '/') p++;
    
    while (*p) {
        /* 找到下一个 / */
        while (*p && *p != '/') p++;
        
        char saved = *p;
        *p = '\0';
        
        /* 尝试创建目录 */
        if (mkdir(tmp, dir_mode) != 0) {
            if (errno != EEXIST) {
                fprintf(stderr, "mkdir: cannot create directory '%s': %s\n", 
                        tmp, strerror(errno));
                ret = 1;
                break;
            }
        }
        
        *p = saved;
        if (saved) p++;
    }
    
    free(tmp);
    return ret;
}

void print_usage(void) {
    printf("Usage: mkdir [OPTION]... DIRECTORY...\n");
    printf("Create the DIRECTORY(ies), if they do not already exist.\n\n");
    printf("Options:\n");
    printf("  -p        no error if existing, make parent directories as needed\n");
    printf("  --help    display this help and exit\n");
}

int main(int argc, char *argv[]) {
    int i;
    int ret = 0;
    int has_dir = 0;
    
    if (argc < 2) {
        fprintf(stderr, "mkdir: missing operand\n");
        fprintf(stderr, "Try 'mkdir --help' for more information.\n");
        return 1;
    }
    
    /* 解析选项 */
    for (i = 1; i < argc; i++) {
        if (argv[i][0] == '-') {
            if (strcmp(argv[i], "--help") == 0) {
                print_usage();
                return 0;
            } else if (strcmp(argv[i], "-p") == 0) {
                opt_parents = 1;
            } else {
                fprintf(stderr, "mkdir: invalid option -- '%s'\n", argv[i]);
                return 1;
            }
        } else {
            has_dir = 1;
        }
    }
    
    if (!has_dir) {
        fprintf(stderr, "mkdir: missing operand\n");
        return 1;
    }
    
    /* 创建目录 */
    for (i = 1; i < argc; i++) {
        if (argv[i][0] == '-') continue;
        
        if (opt_parents) {
            ret |= mkdir_p(argv[i]);
        } else {
            if (mkdir(argv[i], dir_mode) != 0) {
                if (errno == EEXIST) {
                    fprintf(stderr, "mkdir: cannot create directory '%s': File exists\n", argv[i]);
                } else {
                    fprintf(stderr, "mkdir: cannot create directory '%s': %s\n", 
                            argv[i], strerror(errno));
                }
                ret = 1;
            }
        }
    }
    
    return ret;
}
