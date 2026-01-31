/**
 * tree - 以树形结构显示目录内容
 * 用于演示操作系统内核的目录遍历能力 (getdents64)
 */

#define _DEFAULT_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <dirent.h>
#include <sys/stat.h>

#define MAX_PATH 1024
#define MAX_DEPTH 20

/* 统计信息 */
static int dir_count = 0;
static int file_count = 0;

/* 比较函数用于排序 */
int compare_names(const void *a, const void *b) {
    return strcmp(*(const char **)a, *(const char **)b);
}

/* 打印树形结构 */
void print_tree(const char *path, const char *prefix, int is_last, int depth) {
    if (depth > MAX_DEPTH) {
        return;
    }
    
    DIR *dir = opendir(path);
    if (!dir) {
        return;
    }
    
    /* 收集目录项 */
    char *entries[1024];
    int entry_count = 0;
    struct dirent *ent;
    
    while ((ent = readdir(dir)) != NULL && entry_count < 1024) {
        /* 跳过 . 和 .. */
        if (strcmp(ent->d_name, ".") == 0 || strcmp(ent->d_name, "..") == 0) {
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
        int last = (i == entry_count - 1);
        
        /* 构建完整路径 */
        char full_path[MAX_PATH];
        snprintf(full_path, MAX_PATH, "%s/%s", path, entries[i]);
        
        /* 获取文件信息 */
        struct stat st;
        int is_dir = 0;
        if (stat(full_path, &st) == 0) {
            is_dir = S_ISDIR(st.st_mode);
        }
        
        /* 打印树形符号 */
        printf("%s", prefix);
        if (last) {
            printf("└── ");
        } else {
            printf("├── ");
        }
        
        /* 打印名称（目录用蓝色） */
        if (is_dir) {
            printf("\033[34m%s\033[0m\n", entries[i]);
            dir_count++;
            
            /* 递归打印子目录 */
            char new_prefix[MAX_PATH];
            snprintf(new_prefix, MAX_PATH, "%s%s", prefix, last ? "    " : "│   ");
            print_tree(full_path, new_prefix, last, depth + 1);
        } else {
            printf("%s\n", entries[i]);
            file_count++;
        }
        
        free(entries[i]);
    }
}

int main(int argc, char *argv[]) {
    const char *path = ".";
    
    if (argc > 1) {
        path = argv[1];
    }
    
    /* 检查路径是否存在 */
    struct stat st;
    if (stat(path, &st) != 0) {
        fprintf(stderr, "tree: %s: No such file or directory\n", path);
        return 1;
    }
    
    /* 打印根目录 */
    printf("\033[34m%s\033[0m\n", path);
    
    if (S_ISDIR(st.st_mode)) {
        print_tree(path, "", 1, 0);
    }
    
    printf("\n%d directories, %d files\n", dir_count, file_count);
    
    return 0;
}
