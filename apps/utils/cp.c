/**
 * cp - 复制文件
 * 支持 -r (递归复制) 选项
 */

#define _DEFAULT_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <dirent.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <errno.h>

#define BUF_SIZE 8192

static int opt_recursive = 0;  /* -r */

/* 复制单个文件 */
int copy_file(const char *src, const char *dst) {
    int src_fd = open(src, O_RDONLY);
    if (src_fd < 0) {
        fprintf(stderr, "cp: cannot open '%s': %s\n", src, strerror(errno));
        return 1;
    }
    
    /* 获取源文件权限 */
    struct stat st;
    if (fstat(src_fd, &st) != 0) {
        close(src_fd);
        fprintf(stderr, "cp: cannot stat '%s': %s\n", src, strerror(errno));
        return 1;
    }
    
    int dst_fd = open(dst, O_WRONLY | O_CREAT | O_TRUNC, st.st_mode);
    if (dst_fd < 0) {
        close(src_fd);
        fprintf(stderr, "cp: cannot create '%s': %s\n", dst, strerror(errno));
        return 1;
    }
    
    char buf[BUF_SIZE];
    ssize_t n;
    
    while ((n = read(src_fd, buf, BUF_SIZE)) > 0) {
        ssize_t written = 0;
        while (written < n) {
            ssize_t w = write(dst_fd, buf + written, n - written);
            if (w < 0) {
                fprintf(stderr, "cp: write error: %s\n", strerror(errno));
                close(src_fd);
                close(dst_fd);
                return 1;
            }
            written += w;
        }
    }
    
    close(src_fd);
    close(dst_fd);
    
    return n < 0 ? 1 : 0;
}

/* 递归复制目录 */
int copy_dir(const char *src, const char *dst) {
    /* 创建目标目录 */
    struct stat st;
    if (stat(src, &st) != 0) {
        fprintf(stderr, "cp: cannot stat '%s': %s\n", src, strerror(errno));
        return 1;
    }
    
    if (mkdir(dst, st.st_mode) != 0 && errno != EEXIST) {
        fprintf(stderr, "cp: cannot create directory '%s': %s\n", dst, strerror(errno));
        return 1;
    }
    
    DIR *dir = opendir(src);
    if (!dir) {
        fprintf(stderr, "cp: cannot open directory '%s': %s\n", src, strerror(errno));
        return 1;
    }
    
    int ret = 0;
    struct dirent *ent;
    
    while ((ent = readdir(dir)) != NULL) {
        /* 跳过 . 和 .. */
        if (strcmp(ent->d_name, ".") == 0 || strcmp(ent->d_name, "..") == 0) {
            continue;
        }
        
        char src_path[1024], dst_path[1024];
        snprintf(src_path, sizeof(src_path), "%s/%s", src, ent->d_name);
        snprintf(dst_path, sizeof(dst_path), "%s/%s", dst, ent->d_name);
        
        struct stat entry_st;
        if (stat(src_path, &entry_st) != 0) {
            fprintf(stderr, "cp: cannot stat '%s': %s\n", src_path, strerror(errno));
            ret = 1;
            continue;
        }
        
        if (S_ISDIR(entry_st.st_mode)) {
            ret |= copy_dir(src_path, dst_path);
        } else {
            ret |= copy_file(src_path, dst_path);
        }
    }
    
    closedir(dir);
    return ret;
}

/* 获取文件名 */
const char *basename_str(const char *path) {
    const char *p = strrchr(path, '/');
    return p ? p + 1 : path;
}

void print_usage(void) {
    printf("Usage: cp [OPTION]... SOURCE DEST\n");
    printf("       cp [OPTION]... SOURCE... DIRECTORY\n");
    printf("Copy SOURCE to DEST, or multiple SOURCE(s) to DIRECTORY.\n\n");
    printf("Options:\n");
    printf("  -r, -R    copy directories recursively\n");
    printf("  --help    display this help and exit\n");
}

int main(int argc, char *argv[]) {
    int i;
    int first_src = 0;
    int src_count = 0;
    
    if (argc < 3) {
        fprintf(stderr, "cp: missing file operand\n");
        fprintf(stderr, "Try 'cp --help' for more information.\n");
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
                    case 'r':
                    case 'R': opt_recursive = 1; break;
                    default:
                        fprintf(stderr, "cp: invalid option -- '%c'\n", argv[i][j]);
                        return 1;
                }
            }
        } else {
            if (first_src == 0) first_src = i;
            src_count++;
        }
    }
    
    if (src_count < 2) {
        fprintf(stderr, "cp: missing destination file operand after '%s'\n", 
                first_src ? argv[first_src] : "");
        return 1;
    }
    
    /* 找到目标路径 (最后一个非选项参数) */
    char *dest = NULL;
    for (i = argc - 1; i >= first_src; i--) {
        if (argv[i][0] != '-') {
            dest = argv[i];
            break;
        }
    }
    
    struct stat dest_st;
    int dest_is_dir = (stat(dest, &dest_st) == 0 && S_ISDIR(dest_st.st_mode));
    
    /* 如果有多个源文件，目标必须是目录 */
    if (src_count > 2 && !dest_is_dir) {
        fprintf(stderr, "cp: target '%s' is not a directory\n", dest);
        return 1;
    }
    
    int ret = 0;
    
    for (i = first_src; i < argc; i++) {
        if (argv[i][0] == '-' && argv[i][1] != '\0') continue;
        if (argv[i] == dest) continue;
        
        struct stat src_st;
        if (stat(argv[i], &src_st) != 0) {
            fprintf(stderr, "cp: cannot stat '%s': %s\n", argv[i], strerror(errno));
            ret = 1;
            continue;
        }
        
        char final_dest[1024];
        if (dest_is_dir) {
            snprintf(final_dest, sizeof(final_dest), "%s/%s", dest, basename_str(argv[i]));
        } else {
            snprintf(final_dest, sizeof(final_dest), "%s", dest);
        }
        
        if (S_ISDIR(src_st.st_mode)) {
            if (!opt_recursive) {
                fprintf(stderr, "cp: -r not specified; omitting directory '%s'\n", argv[i]);
                ret = 1;
            } else {
                ret |= copy_dir(argv[i], final_dest);
            }
        } else {
            ret |= copy_file(argv[i], final_dest);
        }
    }
    
    return ret;
}
