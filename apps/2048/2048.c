/**
 * 2048 Game - 2048数字游戏
 * 用于演示操作系统内核的终端I/O和键盘输入处理能力
 * 
 * 功能演示:
 * - termios原始模式
 * - 方向键转义序列解析
 * - ANSI彩色输出
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <termios.h>
#include <fcntl.h>
#include <time.h>
#include <sys/time.h>

#define GRID_SIZE 4

/* 游戏状态 */
typedef struct {
    int grid[GRID_SIZE][GRID_SIZE];
    int score;
    int best_score;
    int game_over;
    int won;
} GameState;

static struct termios orig_termios;
static int raw_mode_enabled = 0;

/* 恢复终端设置 */
void disable_raw_mode(void) {
    if (raw_mode_enabled) {
        tcsetattr(STDIN_FILENO, TCSAFLUSH, &orig_termios);
        printf("\033[?25h");  /* 显示光标 */
        printf("\033[0m");    /* 重置颜色 */
        fflush(stdout);
        raw_mode_enabled = 0;
    }
}

/* 启用原始模式 */
void enable_raw_mode(void) {
    struct termios raw;
    
    tcgetattr(STDIN_FILENO, &orig_termios);
    atexit(disable_raw_mode);
    
    raw = orig_termios;
    raw.c_lflag &= ~(ECHO | ICANON | ISIG);
    raw.c_cc[VMIN] = 1;
    raw.c_cc[VTIME] = 0;
    tcsetattr(STDIN_FILENO, TCSAFLUSH, &raw);
    
    printf("\033[?25l");  /* 隐藏光标 */
    fflush(stdout);
    
    raw_mode_enabled = 1;
}

/* 清屏 */
void clear_screen(void) {
    printf("\033[2J\033[H");
    fflush(stdout);
}

/* 根据数字值获取颜色 */
const char* get_color(int value) {
    switch (value) {
        case 0:    return "\033[48;5;250m\033[38;5;250m";  /* 空格 */
        case 2:    return "\033[48;5;255m\033[38;5;0m";    /* 白色背景 */
        case 4:    return "\033[48;5;229m\033[38;5;0m";    /* 浅黄 */
        case 8:    return "\033[48;5;215m\033[38;5;255m";  /* 橙色 */
        case 16:   return "\033[48;5;209m\033[38;5;255m";  /* 深橙 */
        case 32:   return "\033[48;5;203m\033[38;5;255m";  /* 红橙 */
        case 64:   return "\033[48;5;196m\033[38;5;255m";  /* 红色 */
        case 128:  return "\033[48;5;226m\033[38;5;0m";    /* 亮黄 */
        case 256:  return "\033[48;5;220m\033[38;5;0m";    /* 金色 */
        case 512:  return "\033[48;5;214m\033[38;5;0m";    /* 深金 */
        case 1024: return "\033[48;5;208m\033[38;5;255m";  /* 橙金 */
        case 2048: return "\033[48;5;202m\033[38;5;255m";  /* 深橙金 */
        default:   return "\033[48;5;0m\033[38;5;255m";    /* 黑底白字 */
    }
}

/* 重置颜色 */
void reset_color(void) {
    printf("\033[0m");
}

/* 初始化游戏 */
void init_game(GameState *game) {
    memset(game->grid, 0, sizeof(game->grid));
    game->score = 0;
    game->game_over = 0;
    game->won = 0;
}

/* 获取空格子数量 */
int count_empty(GameState *game) {
    int count = 0;
    for (int i = 0; i < GRID_SIZE; i++) {
        for (int j = 0; j < GRID_SIZE; j++) {
            if (game->grid[i][j] == 0) count++;
        }
    }
    return count;
}

/* 随机添加一个数字 (2或4) */
void add_random_tile(GameState *game) {
    int empty = count_empty(game);
    if (empty == 0) return;
    
    int target = rand() % empty;
    int count = 0;
    
    for (int i = 0; i < GRID_SIZE; i++) {
        for (int j = 0; j < GRID_SIZE; j++) {
            if (game->grid[i][j] == 0) {
                if (count == target) {
                    /* 90%概率是2，10%概率是4 */
                    game->grid[i][j] = (rand() % 10 < 9) ? 2 : 4;
                    return;
                }
                count++;
            }
        }
    }
}

/* 检查是否可以移动 */
int can_move(GameState *game) {
    /* 有空格就能移动 */
    if (count_empty(game) > 0) return 1;
    
    /* 检查相邻是否有相同的数字 */
    for (int i = 0; i < GRID_SIZE; i++) {
        for (int j = 0; j < GRID_SIZE; j++) {
            int val = game->grid[i][j];
            if (i < GRID_SIZE - 1 && game->grid[i + 1][j] == val) return 1;
            if (j < GRID_SIZE - 1 && game->grid[i][j + 1] == val) return 1;
        }
    }
    
    return 0;
}

/* 向左移动一行 */
int slide_row_left(int row[GRID_SIZE], int *score) {
    int moved = 0;
    int merged[GRID_SIZE] = {0};  /* 防止连续合并 */
    
    /* 移除空格 */
    int temp[GRID_SIZE] = {0};
    int pos = 0;
    for (int i = 0; i < GRID_SIZE; i++) {
        if (row[i] != 0) {
            if (pos != i) moved = 1;
            temp[pos++] = row[i];
        }
    }
    
    /* 合并相同的数字 */
    for (int i = 0; i < GRID_SIZE - 1; i++) {
        if (temp[i] != 0 && temp[i] == temp[i + 1] && !merged[i]) {
            temp[i] *= 2;
            *score += temp[i];
            temp[i + 1] = 0;
            merged[i] = 1;
            moved = 1;
        }
    }
    
    /* 再次移除空格 */
    pos = 0;
    for (int i = 0; i < GRID_SIZE; i++) {
        row[i] = 0;
    }
    for (int i = 0; i < GRID_SIZE; i++) {
        if (temp[i] != 0) {
            row[pos++] = temp[i];
        }
    }
    
    return moved;
}

/* 向左移动 */
int move_left(GameState *game) {
    int moved = 0;
    for (int i = 0; i < GRID_SIZE; i++) {
        if (slide_row_left(game->grid[i], &game->score)) {
            moved = 1;
        }
    }
    return moved;
}

/* 向右移动 */
int move_right(GameState *game) {
    int moved = 0;
    for (int i = 0; i < GRID_SIZE; i++) {
        /* 反转行 */
        int temp[GRID_SIZE];
        for (int j = 0; j < GRID_SIZE; j++) {
            temp[j] = game->grid[i][GRID_SIZE - 1 - j];
        }
        if (slide_row_left(temp, &game->score)) {
            moved = 1;
        }
        /* 反转回来 */
        for (int j = 0; j < GRID_SIZE; j++) {
            game->grid[i][j] = temp[GRID_SIZE - 1 - j];
        }
    }
    return moved;
}

/* 向上移动 */
int move_up(GameState *game) {
    int moved = 0;
    for (int j = 0; j < GRID_SIZE; j++) {
        /* 提取列 */
        int col[GRID_SIZE];
        for (int i = 0; i < GRID_SIZE; i++) {
            col[i] = game->grid[i][j];
        }
        if (slide_row_left(col, &game->score)) {
            moved = 1;
        }
        /* 放回 */
        for (int i = 0; i < GRID_SIZE; i++) {
            game->grid[i][j] = col[i];
        }
    }
    return moved;
}

/* 向下移动 */
int move_down(GameState *game) {
    int moved = 0;
    for (int j = 0; j < GRID_SIZE; j++) {
        /* 提取列并反转 */
        int col[GRID_SIZE];
        for (int i = 0; i < GRID_SIZE; i++) {
            col[i] = game->grid[GRID_SIZE - 1 - i][j];
        }
        if (slide_row_left(col, &game->score)) {
            moved = 1;
        }
        /* 反转并放回 */
        for (int i = 0; i < GRID_SIZE; i++) {
            game->grid[i][j] = col[GRID_SIZE - 1 - i];
        }
    }
    return moved;
}

/* 检查是否达到2048 */
int check_win(GameState *game) {
    for (int i = 0; i < GRID_SIZE; i++) {
        for (int j = 0; j < GRID_SIZE; j++) {
            if (game->grid[i][j] >= 2048) return 1;
        }
    }
    return 0;
}

/* 渲染游戏界面 */
void render(GameState *game) {
    clear_screen();
    
    printf("\n");
    printf("  +===================================+\n");
    printf("  |            2 0 4 8                |\n");
    printf("  +===================================+\n");
    printf("  |  Score: %-10d Best: %-8d |\n", game->score, game->best_score);
    printf("  +===================================+\n\n");
    
    /* 绘制网格 */
    printf("  +------+------+------+------+\n");
    
    for (int i = 0; i < GRID_SIZE; i++) {
        printf("  |");
        for (int j = 0; j < GRID_SIZE; j++) {
            int val = game->grid[i][j];
            printf("%s", get_color(val));
            if (val == 0) {
                printf("      ");
            } else {
                printf("%5d ", val);
            }
            reset_color();
            printf("|");
        }
        printf("\n");
        
        if (i < GRID_SIZE - 1) {
            printf("  +------+------+------+------+\n");
        }
    }
    
    printf("  +------+------+------+------+\n\n");
    
    printf("  Controls: Arrow Keys / WASD\n");
    printf("  R: Restart | Q: Quit\n");
    
    if (game->won && !game->game_over) {
        printf("\n  *** YOU WIN! *** Press C to continue, R to restart.\n");
    }
    
    if (game->game_over) {
        printf("\n  *** GAME OVER! *** Press R to restart, Q to quit.\n");
    }
    
    fflush(stdout);
}

/* 读取按键 */
int read_key(void) {
    char c;
    if (read(STDIN_FILENO, &c, 1) != 1) return -1;
    
    /* 处理方向键的转义序列 */
    if (c == '\033') {
        char seq[2];
        if (read(STDIN_FILENO, &seq[0], 1) != 1) return c;
        if (read(STDIN_FILENO, &seq[1], 1) != 1) return c;
        if (seq[0] == '[') {
            switch (seq[1]) {
                case 'A': return 'w';  /* 上 */
                case 'B': return 's';  /* 下 */
                case 'C': return 'd';  /* 右 */
                case 'D': return 'a';  /* 左 */
            }
        }
        return c;
    }
    
    return c;
}

int main(void) {
    GameState game;
    
    /* 初始化随机数种子 */
    struct timeval tv;
    gettimeofday(&tv, NULL);
    srand(tv.tv_sec ^ tv.tv_usec);
    
    enable_raw_mode();
    
    /* 显示欢迎界面 */
    clear_screen();
    printf("\n\n");
    printf("  +=======================================+\n");
    printf("  |              2 0 4 8                  |\n");
    printf("  +=======================================+\n");
    printf("  |                                       |\n");
    printf("  |   Join the numbers to get 2048!      |\n");
    printf("  |                                       |\n");
    printf("  |   HOW TO PLAY:                       |\n");
    printf("  |   Use arrow keys or WASD to move     |\n");
    printf("  |   tiles. When two tiles with the     |\n");
    printf("  |   same number touch, they merge      |\n");
    printf("  |   into one!                          |\n");
    printf("  |                                       |\n");
    printf("  |   Press any key to start...          |\n");
    printf("  |                                       |\n");
    printf("  +=======================================+\n");
    fflush(stdout);
    
    read_key();
    
    game.best_score = 0;
    
restart:
    init_game(&game);
    add_random_tile(&game);
    add_random_tile(&game);
    render(&game);
    
    while (1) {
        int key = read_key();
        int moved = 0;
        
        switch (key) {
            case 'w': case 'W':
                moved = move_up(&game);
                break;
            case 's': case 'S':
                moved = move_down(&game);
                break;
            case 'a': case 'A':
                moved = move_left(&game);
                break;
            case 'd': case 'D':
                moved = move_right(&game);
                break;
            case 'r': case 'R':
                goto restart;
            case 'q': case 'Q':
                goto quit;
            case 'c': case 'C':
                if (game.won) {
                    game.won = 0;  /* 继续游戏 */
                }
                break;
        }
        
        if (moved && !game.game_over) {
            add_random_tile(&game);
            
            /* 更新最高分 */
            if (game.score > game.best_score) {
                game.best_score = game.score;
            }
            
            /* 检查胜利 */
            if (!game.won && check_win(&game)) {
                game.won = 1;
            }
            
            /* 检查游戏结束 */
            if (!can_move(&game)) {
                game.game_over = 1;
            }
            
            render(&game);
        }
    }

quit:
    clear_screen();
    printf("\n  Thanks for playing 2048!\n");
    printf("  Final Score: %d\n", game.score);
    printf("  Best Score: %d\n\n", game.best_score);
    
    return 0;
}
