/**
 * Snake Game - 贪吃蛇游戏
 * 用于演示操作系统内核的终端I/O和进程管理能力
 * 
 * 功能演示:
 * - termios原始模式 (tcgetattr/tcsetattr)
 * - 非阻塞I/O (fcntl)
 * - ANSI转义序列渲染
 * - 定时器 (usleep)
 * - 随机数生成
 */

#define _DEFAULT_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <termios.h>
#include <fcntl.h>
#include <time.h>
#include <sys/time.h>

/* 游戏区域大小 */
#define WIDTH  40
#define HEIGHT 20
#define MAX_SNAKE_LEN (WIDTH * HEIGHT)

/* 方向定义 */
enum Direction { UP, DOWN, LEFT, RIGHT };

/* 坐标结构 */
typedef struct {
    int x, y;
} Point;

/* 游戏状态 */
typedef struct {
    Point snake[MAX_SNAKE_LEN];
    int snake_len;
    enum Direction dir;
    Point food;
    int score;
    int game_over;
    int speed;  /* 毫秒 */
} GameState;

static struct termios orig_termios;
static int raw_mode_enabled = 0;

/* 恢复终端设置 */
void disable_raw_mode(void) {
    if (raw_mode_enabled) {
        tcsetattr(STDIN_FILENO, TCSAFLUSH, &orig_termios);
        /* 显示光标 */
        printf("\033[?25h");
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
    raw.c_cc[VMIN] = 0;
    raw.c_cc[VTIME] = 0;
    tcsetattr(STDIN_FILENO, TCSAFLUSH, &raw);
    
    /* 设置非阻塞读取 */
    int flags = fcntl(STDIN_FILENO, F_GETFL, 0);
    fcntl(STDIN_FILENO, F_SETFL, flags | O_NONBLOCK);
    
    /* 隐藏光标 */
    printf("\033[?25l");
    fflush(stdout);
    
    raw_mode_enabled = 1;
}

/* 清屏 */
void clear_screen(void) {
    printf("\033[2J\033[H");
    fflush(stdout);
}

/* 移动光标 */
void move_cursor(int x, int y) {
    printf("\033[%d;%dH", y + 1, x + 1);
}

/* 设置颜色 */
void set_color(int fg) {
    printf("\033[%dm", fg);
}

/* 重置颜色 */
void reset_color(void) {
    printf("\033[0m");
}

/* 获取随机数 */
int get_random(int max) {
    return rand() % max;
}

/* 生成食物位置 */
void spawn_food(GameState *game) {
    int valid;
    do {
        valid = 1;
        game->food.x = get_random(WIDTH - 2) + 1;
        game->food.y = get_random(HEIGHT - 2) + 1;
        
        /* 检查是否与蛇身重叠 */
        for (int i = 0; i < game->snake_len; i++) {
            if (game->snake[i].x == game->food.x && 
                game->snake[i].y == game->food.y) {
                valid = 0;
                break;
            }
        }
    } while (!valid);
}

/* 初始化游戏 */
void init_game(GameState *game) {
    memset(game, 0, sizeof(GameState));
    
    /* 蛇初始位置在中间 */
    game->snake_len = 3;
    game->snake[0].x = WIDTH / 2;
    game->snake[0].y = HEIGHT / 2;
    game->snake[1].x = WIDTH / 2 - 1;
    game->snake[1].y = HEIGHT / 2;
    game->snake[2].x = WIDTH / 2 - 2;
    game->snake[2].y = HEIGHT / 2;
    
    game->dir = RIGHT;
    game->score = 0;
    game->game_over = 0;
    game->speed = 150;  /* 150ms */
    
    spawn_food(game);
}

/* Draw game screen */
void render(GameState *game) {
    clear_screen();
    
    /* Draw border */
    set_color(36);  /* Cyan */
    for (int x = 0; x < WIDTH; x++) {
        move_cursor(x, 0);
        printf("#");
        move_cursor(x, HEIGHT - 1);
        printf("#");
    }
    for (int y = 0; y < HEIGHT; y++) {
        move_cursor(0, y);
        printf("#");
        move_cursor(WIDTH - 1, y);
        printf("#");
    }
    
    /* 绘制蛇 */
    for (int i = 0; i < game->snake_len; i++) {
        move_cursor(game->snake[i].x, game->snake[i].y);
        if (i == 0) {
            set_color(32);  /* 绿色蛇头 */
            printf("@");
        } else {
            set_color(92);  /* 亮绿色蛇身 */
            printf("o");
        }
    }
    
    /* 绘制食物 */
    set_color(31);  /* 红色 */
    move_cursor(game->food.x, game->food.y);
    printf("*");
    
    /* 绘制分数 */
    reset_color();
    move_cursor(0, HEIGHT + 1);
    printf("Score: %d  |  Speed: %dms  |  Length: %d", 
           game->score, game->speed, game->snake_len);
    move_cursor(0, HEIGHT + 2);
    printf("Controls: WASD or Arrow Keys | Q: Quit");
    
    fflush(stdout);
}

/* 处理输入 */
void handle_input(GameState *game) {
    char c;
    while (read(STDIN_FILENO, &c, 1) == 1) {
        /* 处理方向键的转义序列 */
        if (c == '\033') {
            char seq[2];
            if (read(STDIN_FILENO, &seq[0], 1) != 1) continue;
            if (read(STDIN_FILENO, &seq[1], 1) != 1) continue;
            if (seq[0] == '[') {
                switch (seq[1]) {
                    case 'A': c = 'w'; break;  /* 上 */
                    case 'B': c = 's'; break;  /* 下 */
                    case 'C': c = 'd'; break;  /* 右 */
                    case 'D': c = 'a'; break;  /* 左 */
                }
            }
        }
        
        switch (c) {
            case 'w': case 'W':
                if (game->dir != DOWN) game->dir = UP;
                break;
            case 's': case 'S':
                if (game->dir != UP) game->dir = DOWN;
                break;
            case 'a': case 'A':
                if (game->dir != RIGHT) game->dir = LEFT;
                break;
            case 'd': case 'D':
                if (game->dir != LEFT) game->dir = RIGHT;
                break;
            case 'q': case 'Q':
                game->game_over = 1;
                break;
        }
    }
}

/* 更新游戏状态 */
void update(GameState *game) {
    /* 计算新头部位置 */
    Point new_head = game->snake[0];
    
    switch (game->dir) {
        case UP:    new_head.y--; break;
        case DOWN:  new_head.y++; break;
        case LEFT:  new_head.x--; break;
        case RIGHT: new_head.x++; break;
    }
    
    /* 碰撞检测：边界 */
    if (new_head.x <= 0 || new_head.x >= WIDTH - 1 ||
        new_head.y <= 0 || new_head.y >= HEIGHT - 1) {
        game->game_over = 1;
        return;
    }
    
    /* 碰撞检测：自身 */
    for (int i = 0; i < game->snake_len; i++) {
        if (game->snake[i].x == new_head.x && 
            game->snake[i].y == new_head.y) {
            game->game_over = 1;
            return;
        }
    }
    
    /* 检测是否吃到食物 */
    int ate_food = (new_head.x == game->food.x && new_head.y == game->food.y);
    
    /* 移动蛇 */
    if (!ate_food) {
        /* 没吃到食物，移除尾巴 */
        for (int i = game->snake_len - 1; i > 0; i--) {
            game->snake[i] = game->snake[i - 1];
        }
    } else {
        /* 吃到食物，增长蛇身 */
        for (int i = game->snake_len; i > 0; i--) {
            game->snake[i] = game->snake[i - 1];
        }
        game->snake_len++;
        game->score += 10;
        
        /* 加速 */
        if (game->speed > 50) {
            game->speed -= 5;
        }
        
        spawn_food(game);
    }
    
    game->snake[0] = new_head;
}

/* Game over screen */
void show_game_over(GameState *game) {
    clear_screen();
    printf("\n\n");
    printf("   +------------------------------+\n");
    printf("   |        GAME OVER!            |\n");
    printf("   +------------------------------+\n");
    printf("   |  Final Score: %-14d |\n", game->score);
    printf("   |  Snake Length: %-13d |\n", game->snake_len);
    printf("   +------------------------------+\n");
    printf("   |  Press R to restart          |\n");
    printf("   |  Press Q to quit             |\n");
    printf("   +------------------------------+\n");
    fflush(stdout);
}

/* 获取当前时间(毫秒) */
long get_time_ms(void) {
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return tv.tv_sec * 1000 + tv.tv_usec / 1000;
}

int main(void) {
    GameState game;
    long last_update;
    
    /* 初始化随机数种子 */
    struct timeval tv;
    gettimeofday(&tv, NULL);
    srand(tv.tv_sec ^ tv.tv_usec);
    
    enable_raw_mode();
    
    /* Show welcome screen */
    clear_screen();
    printf("\n\n");
    printf("   +--------------------------------------+\n");
    printf("   |           SNAKE GAME                 |\n");
    printf("   +--------------------------------------+\n");
    printf("   |                                      |\n");
    printf("   |   Controls:                          |\n");
    printf("   |     W : Move Up                      |\n");
    printf("   |     S : Move Down                    |\n");
    printf("   |     A : Move Left                    |\n");
    printf("   |     D : Move Right                   |\n");
    printf("   |     Q : Quit                         |\n");
    printf("   |                                      |\n");
    printf("   |   Eat * to grow longer!              |\n");
    printf("   |   Don't hit the walls or yourself!  |\n");
    printf("   |                                      |\n");
    printf("   |   Press any key to start...         |\n");
    printf("   +--------------------------------------+\n");
    fflush(stdout);
    
    /* 等待按键 */
    while (1) {
        char c;
        if (read(STDIN_FILENO, &c, 1) == 1) break;
        usleep(10000);
    }

restart:
    init_game(&game);
    last_update = get_time_ms();
    
    /* 主游戏循环 */
    while (!game.game_over) {
        handle_input(&game);
        
        long now = get_time_ms();
        if (now - last_update >= game.speed) {
            update(&game);
            render(&game);
            last_update = now;
        }
        
        usleep(10000);  /* 10ms 轮询间隔 */
    }
    
    /* 游戏结束 */
    show_game_over(&game);
    
    while (1) {
        char c;
        if (read(STDIN_FILENO, &c, 1) == 1) {
            if (c == 'r' || c == 'R') {
                goto restart;
            }
            if (c == 'q' || c == 'Q') {
                break;
            }
        }
        usleep(10000);
    }
    
    clear_screen();
    printf("Thanks for playing Snake!\n");
    
    return 0;
}
