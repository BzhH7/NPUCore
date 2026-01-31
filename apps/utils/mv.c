/**
 * mv - 移动或重命名文件
 */

#define _DEFAULT_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <errno.h>

#define BUF_SIZE 8192

/* 获取文件名 */
const char *basename_str(const char *path) {
    const char *p = strrchr(path, '/');
    return p ? p + 1 : path;
}

/* 复制文件内容 (用于跨文件系统移动) */
int copy_and_remove(const char *src, const char *dst) {
    struct stat st;
    if (stat(src, &st) != 0) {
        fprintf(stderr, "mv: cannot stat '%s': %s\n", src, strerror(errno));
        return 1;
    }
    
    int src_fd = open(src, O_RDONLY);
    if (src_fd < 0) {
        fprintf(stderr, "mv: cannot open '%s': %s\n", src, strerror(errno));
        return 1;
    }
    
    int dst_fd = open(dst, O_WRONLY | O_CREAT | O_TRUNC, st.st_mode);
    if (dst_fd < 0) {
        close(src_fd);
        fprintf(stderr, "mv: cannot create '%s': %s\n", dst, strerror(errno));
        return 1;
    }
    
    char buf[BUF_SIZE];
    ssize_t n;
    int ret = 0;
    
    while ((n = read(src_fd, buf, BUF_SIZE)) > 0) {
        ssize_t written = 0;
        while (written < n) {
            ssize_t w = write(dst_fd, buf + written, n - written);
            if (w < 0) {
                fprintf(stderr, "mv: write error: %s\n", strerror(errno));
                ret = 1;
                goto cleanup;
            }
            written += w;
        }
    }
    
    if (n < 0) {
        fprintf(stderr, "mv: read error: %s\n", strerror(errno));
        ret = 1;
        goto cleanup;
    }
    
    /* 删除源文件 */
    if (unlink(src) != 0) {
        fprintf(stderr, "mv: cannot remove '%s': %s\n", src, strerror(errno));
        ret = 1;
    }
    
cleanup:
    close(src_fd);
    close(dst_fd);
    return ret;
}

/* 移动文件 */
int move_file(const char *src, const char *dst) {
    /* 首先尝试 rename */
    if (rename(src, dst) == 0) {
        return 0;
    }
    
    /* 如果 rename 失败 (可能跨文件系统)，尝试复制后删除 */
    if (errno == EXDEV) {
        return copy_and_remove(src, dst);
    }
    
    fprintf(stderr, "mv: cannot move '%s' to '%s': %s\n", src, dst, strerror(errno));
    return 1;
}

void print_usage(void) {
    printf("Usage: mv [OPTION]... SOURCE DEST\n");
    printf("       mv [OPTION]... SOURCE... DIRECTORY\n");
    printf("Rename SOURCE to DEST, or move SOURCE(s) to DIRECTORY.\n\n");
    printf("Options:\n");
    printf("  --help    display this help and exit\n");
}

int main(int argc, char *argv[]) {
    int i;
    int first_src = 0;
    int src_count = 0;
    
    if (argc < 3) {
        fprintf(stderr, "mv: missing file operand\n");
        fprintf(stderr, "Try 'mv --help' for more information.\n");
        return 1;
    }
    
    /* 解析选项 */
    for (i = 1; i < argc; i++) {
        if (argv[i][0] == '-' && argv[i][1] != '\0') {
            if (strcmp(argv[i], "--help") == 0) {
                print_usage();
                return 0;
            }
            fprintf(stderr, "mv: invalid option -- '%s'\n", argv[i]);
            return 1;
        } else {
            if (first_src == 0) first_src = i;
            src_count++;
        }
    }
    
    if (src_count < 2) {
        fprintf(stderr, "mv: missing destination file operand after '%s'\n", 
                first_src ? argv[first_src] : "");
        return 1;
    }
    
    /* 目标是最后一个参数 */
    char *dest = argv[argc - 1];
    
    struct stat dest_st;
    int dest_is_dir = (stat(dest, &dest_st) == 0 && S_ISDIR(dest_st.st_mode));
    
    /* 如果有多个源文件，目标必须是目录 */
    if (src_count > 2 && !dest_is_dir) {
        fprintf(stderr, "mv: target '%s' is not a directory\n", dest);
        return 1;
    }
    
    int ret = 0;
    
    for (i = first_src; i < argc - 1; i++) {
        struct stat src_st;
        if (stat(argv[i], &src_st) != 0) {
            fprintf(stderr, "mv: cannot stat '%s': %s\n", argv[i], strerror(errno));
            ret = 1;
            continue;
        }
        
        char final_dest[1024];
        if (dest_is_dir) {
            snprintf(final_dest, sizeof(final_dest), "%s/%s", dest, basename_str(argv[i]));
        } else {
            snprintf(final_dest, sizeof(final_dest), "%s", dest);
        }
        
        ret |= move_file(argv[i], final_dest);
    }
    
    return ret;
}
