/**
 * rm - 删除文件或目录
 * 支持 -r (递归删除) 和 -f (强制删除) 选项
 */

#define _DEFAULT_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <dirent.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <errno.h>

static int opt_recursive = 0;  /* -r */
static int opt_force = 0;      /* -f */

/* 递归删除目录 */
int remove_dir(const char *path) {
    DIR *dir = opendir(path);
    if (!dir) {
        if (!opt_force) {
            fprintf(stderr, "rm: cannot open directory '%s': %s\n", path, strerror(errno));
        }
        return opt_force ? 0 : 1;
    }
    
    int ret = 0;
    struct dirent *ent;
    
    while ((ent = readdir(dir)) != NULL) {
        /* 跳过 . 和 .. */
        if (strcmp(ent->d_name, ".") == 0 || strcmp(ent->d_name, "..") == 0) {
            continue;
        }
        
        char full_path[1024];
        snprintf(full_path, sizeof(full_path), "%s/%s", path, ent->d_name);
        
        struct stat st;
        if (lstat(full_path, &st) != 0) {
            if (!opt_force) {
                fprintf(stderr, "rm: cannot stat '%s': %s\n", full_path, strerror(errno));
                ret = 1;
            }
            continue;
        }
        
        if (S_ISDIR(st.st_mode)) {
            ret |= remove_dir(full_path);
        } else {
            if (unlink(full_path) != 0) {
                if (!opt_force) {
                    fprintf(stderr, "rm: cannot remove '%s': %s\n", full_path, strerror(errno));
                    ret = 1;
                }
            }
        }
    }
    
    closedir(dir);
    
    /* 删除空目录 */
    if (rmdir(path) != 0) {
        if (!opt_force) {
            fprintf(stderr, "rm: cannot remove '%s': %s\n", path, strerror(errno));
            ret = 1;
        }
    }
    
    return ret;
}

/* 删除文件或目录 */
int remove_path(const char *path) {
    struct stat st;
    
    if (lstat(path, &st) != 0) {
        if (!opt_force) {
            fprintf(stderr, "rm: cannot remove '%s': No such file or directory\n", path);
            return 1;
        }
        return 0;
    }
    
    if (S_ISDIR(st.st_mode)) {
        if (!opt_recursive) {
            fprintf(stderr, "rm: cannot remove '%s': Is a directory\n", path);
            return 1;
        }
        return remove_dir(path);
    }
    
    if (unlink(path) != 0) {
        if (!opt_force) {
            fprintf(stderr, "rm: cannot remove '%s': %s\n", path, strerror(errno));
            return 1;
        }
    }
    
    return 0;
}

void print_usage(void) {
    printf("Usage: rm [OPTION]... FILE...\n");
    printf("Remove (unlink) the FILE(s).\n\n");
    printf("Options:\n");
    printf("  -f        ignore nonexistent files, never prompt\n");
    printf("  -r, -R    remove directories and their contents recursively\n");
    printf("  --help    display this help and exit\n");
}

int main(int argc, char *argv[]) {
    int i;
    int ret = 0;
    int has_file = 0;
    
    if (argc < 2) {
        fprintf(stderr, "rm: missing operand\n");
        fprintf(stderr, "Try 'rm --help' for more information.\n");
        return 1;
    }
    
    /* 解析选项 */
    for (i = 1; i < argc; i++) {
        if (argv[i][0] == '-' && argv[i][1] != '\0') {
            if (strcmp(argv[i], "--help") == 0) {
                print_usage();
                return 0;
            }
            for (int j = 1; argv[i][j]; j++) {
                switch (argv[i][j]) {
                    case 'f': opt_force = 1; break;
                    case 'r':
                    case 'R': opt_recursive = 1; break;
                    default:
                        fprintf(stderr, "rm: invalid option -- '%c'\n", argv[i][j]);
                        return 1;
                }
            }
        } else {
            has_file = 1;
        }
    }
    
    if (!has_file && !opt_force) {
        fprintf(stderr, "rm: missing operand\n");
        return 1;
    }
    
    /* 删除文件 */
    for (i = 1; i < argc; i++) {
        if (argv[i][0] == '-' && argv[i][1] != '\0') continue;
        ret |= remove_path(argv[i]);
    }
    
    return ret;
}
