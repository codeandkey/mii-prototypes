/*
 * lmc.c
 *
 * lightweight module cache
 */

/* includes */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <math.h>

#include <dirent.h>
#include <errno.h>
#include <unistd.h>

#include <sys/stat.h>
#include <sys/types.h>

#include <sqlite3.h>

/* string list type */
typedef struct {
    char** list;
    int len;
} string_list;

/* compile-time constants */
#define HOME_DATA_SUFFIX ".cache/lmc"

/* options / paths */
static char* data_dir;    /* default: $HOME/.cache/lmc or /tmp/lmc.XXXX if HOME is not set */
static char* module_path; /* default: $MODULEPATH */

/* globals */
static sqlite3*    db_connection;
static string_list module_roots;

/* db functions */
int  db_init();
void db_free();
int  db_flush_binaries();

/* crawling functions */
int build_root(char* root);

/* util functions */
int   init_datapath(char* user_path);   /* initialize data paths */
void  init_modulepath(char* user_path); /* initialize module paths */
int   path_try(char* path);             /* verify a path can be used as a directory */
char* join_path(char* a, char* b);      /* join two paths */

/* string_list functions */
void string_list_append(string_list* l, char* item);
void string_list_free(string_list* l);

/*
 * main(...)
 *
 * program entry point
 */
int main(int argc, char** argv) {
    /* seed prng */
    srand(time(NULL));

    if (init_datapath(NULL)) {
        fprintf(stderr, "error: couldn't initialize any valid data directories!\n");
        return EXIT_FAILURE;
    }

    fprintf(stderr, "note: proceeding with verified data directory %s\n", data_dir);

    init_modulepath(NULL);

    if (db_init()) {
        return -1;
    }

    if (db_flush_binaries()) return -1;

    db_free();
    fprintf(stderr, "Bye\n");
    return 0;
}

/*
 * db_init()
 *
 * initializes the local database at "<cache_path>/lmc.db"
 * also creates the table structure for the cache if it does not exist
 * returns nonzero on failure
 */
int db_init() {
    char* db_path, *sql_error;
    int res;

    db_path = join_path(data_dir, "lmc.db");
    res = sqlite3_open(db_path, &db_connection);

    free(db_path);

    if (res) {
        fprintf(stderr, "error: failed to open database at %s: %s\n", db_path, sqlite3_errmsg(db_connection));
        return -1;
    }

    /* create binaries table */
    if (sqlite3_exec(db_connection, "create table if not exists binaries (root text, module_code text, bin_name tinytext)",
                     NULL, NULL, &sql_error)) {
        fprintf(stderr, "error: failed to initialize binaries table: %s\n", sql_error);
        return -1;
    }

    return 0;
}

/*
 * db_free()
 *
 * terminates the database connection
 */
void db_free() {
    sqlite3_close(db_connection);
    db_connection = NULL;
}

/*
 * db_flush_binaries()
 *
 * clear all binary entries from the database
 */
int db_flush_binaries() {
    char* sql_error;

    if (sqlite3_exec(db_connection, "delete from binaries", NULL, NULL, &sql_error)) {
        fprintf(stderr, "error: failed to initialize binaries table: %s\n", sql_error);
        return -1;
    }

    fprintf(stderr, "note: flushed all binaries from database\n");
    return 0;
}

/*
 * init_datapath(user_path)
 *
 * tries to initialize the lmc data directory
 * returns nonzero if no valid path could be initialized
 *
 * user_path: NULL or path to prefer over $HOME/.cache/lmc
 *
 * precedence:
 *   user_path
 *   $HOME/.cache/lmc
 *   /tmp/lmcXXXX
 */
int init_datapath(char* user_path) {
    char* home_env = getenv("HOME");

    if (user_path && !path_try(user_path)) {
        data_dir = user_path;
        return 0;
    }

    if (home_env && strlen(home_env)) {
        char* home_data = join_path(home_env, HOME_DATA_SUFFIX);

        if (!path_try(home_data)) {
            data_dir = home_data;
            return 0;
        } else {
            free(home_data);
        }
    } else {
        fprintf(stderr, "warning: HOME variable not set!\n");
    }

    data_dir = malloc(13);
    snprintf(data_dir, 13, "/tmp/lmc%04x", rand());
    return path_try(data_dir);
}

/*
 * init_modulepath(user_path)
 *
 * initialize and verify module paths
 *
 * user_path: MODULEPATH override
 */
void init_modulepath(char* user_path) {
    char* cur_path, *tmp_module_path;

    if (user_path) {
        module_path = user_path;
    } else {
        if (!(module_path = getenv("MODULEPATH"))) {
            fprintf(stderr, "warning: MODULEPATH not set\n");
            module_path = "";
        }
    }

    if (!strlen(module_path)) {
        fprintf(stderr, "warning: no module paths, will not be able to find modules\n");
        return;
    }

    tmp_module_path = strdup(module_path);
    for (cur_path = strtok(tmp_module_path, ":"); cur_path; cur_path = strtok(NULL, ":")) {
        string_list_append(&module_roots, cur_path);
        fprintf(stderr, "note: using module root %s\n", cur_path);
    }
    free(tmp_module_path);
}

/*
 * path_try(path)
 *
 * verifies that a path can be used as a directory by lmc
 * will try and create it if it does not exist
 *
 * path: path to test
 *
 * returns nonzero if the path cannot be used
 */
int path_try(char* path) {
    struct stat st;
    DIR* d;

    if (mkdir(path, 0755) && errno != EEXIST) {
        fprintf(stderr, "warning: mkdir() failed for %s: %m\n", path);
        return -1;
    }

    /* verify that the path is a directory and we have permissions on it */

    if (stat(path, &st)) {
        fprintf(stderr, "warning: stat() failed for %s: %m\n", path);
        return -1;
    }

    if (!(d = opendir(path))) {
        fprintf(stderr, "warning: opendir() failed for %s: %m\n", path);
        return -1;
    }

    closedir(d);
    return 0;
}

/*
 * join_path(a, b)
 *
 * joins a and b together. there is no escaping or safety checks
 * returns a dynamically allocated string with the joined path
 *
 * a: first path
 * b: second path
 *
 * the result should be passed to free() after use
 */
char* join_path(char* a, char* b) {
    int outsize = strlen(a) + strlen(b) + 2;
    char* out = malloc(outsize);
    snprintf(out, outsize, "%s/%s", a, b);
    return out;
}

/*
 * string_list_append(list, item)
 *
 * appends a string to a list
 * the string is copied and seperately allocated before it is pushed on the list
 *
 * list: the list to append to
 * item: the item to append
 */
void string_list_append(string_list* l, char* item) {
    l->len++;
    l->list = realloc(l->list, l->len * sizeof(char*));
    l->list[l->len - 1] = item;
}

/*
 * build_root(root)
 * rebuilds the cache for a module root
 *
 * root: module root to build
 *
 * returns nonzero if something goes wrong
 */
int build_root(char* root) {
    /*
     * first, we must remove any binary entries from this root.
     * this can be done with a single prepared statement.
     */

    return 0;
}

/*
 * string_list_free(list)
 *
 * frees all elements and the list itself
 *
 * list: the list to free
 */
void string_list_free(string_list* l) {
    while (l->len) free(l->list[--l->len]);
    free(l->list);
}
