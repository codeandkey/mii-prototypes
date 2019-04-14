/*
 * lmc.c
 *
 * lightweight module cache
 */

/* enable POSIX extensions strdup() and setenv() */
#define _BSD_SOURCE
#define _DEFAULT_SOURCE

/* includes */
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <time.h>
#include <math.h>

#include <getopt.h>
#include <dirent.h>
#include <errno.h>
#include <unistd.h>
#include <regex.h>
#include <wordexp.h>

#include <sys/stat.h>
#include <sys/types.h>

#include <sqlite3.h>

/* string list type */
typedef struct {
    char** list;
    int len;
} string_list;

/* search result type */
typedef struct {
    string_list roots, codes, bins;
} search_result;

/* compile-time constants */
#define HOME_DATA_SUFFIX ".cache/lmc"
#define LINEBUF_SIZE 512

/* options / paths */
static char* data_dir;    /* default: $HOME/.cache/lmc or /tmp/lmc.XXXX if HOME is not set */
static char* module_path; /* default: $MODULEPATH */
static int   verbose;
static int   datapath_should_free;

/* globals */
static sqlite3*    db_connection;
static string_list module_roots;
static int         module_count;

/* prepared statements */
static const char* stmt_src_add_bin = "insert into binaries values (?, ?, ?)";
static const char* stmt_src_search_bin_exact = "select * from binaries where bin=?";
static const char* stmt_src_search_bin_similar = "select * from binaries where bin like ?";

static sqlite3_stmt* stmt_add_bin;
static sqlite3_stmt* stmt_search_bin_exact;
static sqlite3_stmt* stmt_search_bin_similar;

/* regular expressions */
static const char* reg_src_lmod = "^[[:space:]]*(prepend_path|append_path)" \
                                  "[[:space:]]*\\([[:space:]]*\"PATH\"[[:space:]]*" \
                                  ",[[:space:]]*\"([^\"]+)\"[[:space:]]*(,[[:space:]]*\"[^\"]*)\"[[:space:]]*)?\\)[[:space:]]*$";

static regex_t reg_lmod;

/* db functions */
int  db_init();
void db_free();
int  db_begin_transaction();
int  db_end_transaction();
int  db_flush_binaries();

/* crawling functions */
int build_root(char* root);
int build_module_dir(char* root, char* module_dir, char* module_name);
int build_module_file(char* root, char* module_name, char* module_file_name, char* module_file_path);
int build_potential_path(char* root, char* code, char* path);

/* parsing functions */
int extract_lmod(char* path, string_list* list);
int extract_tcl(char* path, string_list* list);

/* searching functions */
search_result search_binary(char* bin);
search_result search_similar(char* bin);

/* util functions */
int   init_regex();                     /* initialize regular expressions */
int   init_datapath(char* user_path);   /* initialize data paths */
void  free_datapath();                  /* free data paths */
void  init_modulepath(char* user_path); /* initialize module paths */
void  free_modulepath();                /* free module paths */
void  free_regex();                     /* free compiled regular expressions */
int   path_try(char* path);             /* verify a path can be used as a directory */
char* join_path(char* a, char* b);      /* join two paths */
char* expand_string(char* str);         /* expand string with environment substitutions */
void  usage(char* a0);                  /* print usage info */

/* string_list functions */
void string_list_append(string_list* l, char* item);
void string_list_free(string_list* l);

/* search_result functions */
void search_result_append(search_result* list, char* root, char* code, char* bin);
void search_result_free(search_result* list);

/*
 * main(...)
 *
 * program entry point
 */
int main(int argc, char** argv) {
    int opt;
    char* user_datapath = NULL, *user_modulepath = NULL, *subcommand = NULL;

    while ((opt = getopt(argc, argv, "d:m:v")) != -1) {
        switch (opt) {
        case 'd':
            user_datapath = optarg;
            break;
        case 'm':
            user_modulepath = optarg;
            break;
        case 'v':
            verbose = 1;
            break;
        default:
            fprintf(stderr, "error: unrecognized option '%c'\n", opt);
        case '?':
            usage(*argv);
            return -1;
        }
    }

    if (optind < argc) {
        subcommand = argv[optind];
    } else {
        usage(*argv);
        return -1;
    }

    /* seed prng */
    srand(time(NULL));

    if (init_datapath(user_datapath)) {
        fprintf(stderr, "error: couldn't initialize any valid data directories!\n");
        return EXIT_FAILURE;
    }

    if (verbose) fprintf(stderr, "note: proceeding with verified data directory %s\n", data_dir);

    init_modulepath(user_modulepath);

    if (db_init()) {
        return EXIT_FAILURE;
    }

    if (!strcmp(subcommand, "help")) {
        fprintf(stderr, "lmc: lightweight module cache\n\n");
        usage(*argv);
        return 0;
    } else if (!strcmp(subcommand, "build")) {
        /* regexes are only used in the build process,
         * so we compile and free them in this subcommand */
        if (init_regex()) {
            return EXIT_FAILURE;
        }

        module_count = 0;
        clock_t begin = clock();

        if (db_begin_transaction()) return -1;
        if (db_flush_binaries()) return -1;

        for (int i = 0; i < module_roots.len; ++i) {
            build_root(module_roots.list[i]);
        }

        if (db_end_transaction()) return -1;
        clock_t end = clock();
        fprintf(stderr, "lmc: cached %d modules in %.2f seconds\n", module_count, (float) (end - begin) / (float) CLOCKS_PER_SEC);
        free_regex();
    } else if (!strcmp(subcommand, "search")) {
        if (++optind >= argc) {
            db_free();
            usage(*argv);
            return -1;
        }

        search_result res = search_binary(argv[optind]);

        for (int i = 0; i < res.bins.len; ++i) {
            printf("=> root=\"%s\", code=\"%s\", bin=\"%s\"\n", res.roots.list[i], res.codes.list[i], res.bins.list[i]);
        }

        search_result_free(&res);
    } else if (!strcmp(subcommand, "like")) {
        if (++optind >= argc) {
            db_free();
            usage(*argv);
            return -1;
        }

        search_result res = search_similar(argv[optind]);

        for (int i = 0; i < res.bins.len; ++i) {
            printf("=> root=\"%s\", code=\"%s\", bin=\"%s\"\n", res.roots.list[i], res.codes.list[i], res.bins.list[i]);
        }

        search_result_free(&res);
    } else {
        db_free();
        fprintf(stderr, "error: invalid subcommand %s\n", subcommand);
        usage(*argv);
        return -1;
    }

    free_datapath();
    free_modulepath();
    db_free();
    if (verbose) fprintf(stderr, "Bye\n");
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
    if (sqlite3_exec(db_connection, "create table if not exists binaries (root text, code text, bin tinytext)",
                     NULL, NULL, &sql_error)) {
        fprintf(stderr, "error: failed to initialize binaries table: %s\n", sql_error);
        return -1;
    }

    /* create prepared statements */
    if (sqlite3_prepare_v2(db_connection, stmt_src_add_bin, -1, &stmt_add_bin, NULL)) {
        fprintf(stderr, "error: failed to initialize add_bin statement: %s\n", sqlite3_errmsg(db_connection));
        return -1;
    }

    if (sqlite3_prepare_v2(db_connection, stmt_src_search_bin_exact, -1, &stmt_search_bin_exact, NULL)) {
        fprintf(stderr, "error: failed to initialize search_bin_exact statement: %s\n", sqlite3_errmsg(db_connection));
        return -1;
    }

    if (sqlite3_prepare_v2(db_connection, stmt_src_search_bin_similar, -1, &stmt_search_bin_similar, NULL)) {
        fprintf(stderr, "error: failed to initialize search_bin_similar statement: %s\n", sqlite3_errmsg(db_connection));
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
    /* free prepared statements */
    sqlite3_finalize(stmt_add_bin);
    sqlite3_finalize(stmt_search_bin_exact);
    sqlite3_finalize(stmt_search_bin_similar);

    /* close connection */
    sqlite3_close(db_connection);
    db_connection = NULL;
}

/*
 * db_begin_transaction()
 * start a database transaction
 *
 * should be called before rebuilding the cache
 * returns nonzero if bad things happen
 */
int db_begin_transaction() {
    char* sql_error;

    if (sqlite3_exec(db_connection, "begin transaction", NULL, NULL, &sql_error)) {
        fprintf(stderr, "error: failed to begin transaction: %s\n", sql_error);
        return -1;
    }

    return 0;
}

/*
 * db_end_transaction()
 * end a database transaction
 *
 * should be after rebuilding the cache
 * returns nonzero if bad things happen
 */
int db_end_transaction() {
    char* sql_error;

    if (sqlite3_exec(db_connection, "end transaction", NULL, NULL, &sql_error)) {
        fprintf(stderr, "error: failed to end transaction: %s\n", sql_error);
        return -1;
    }

    return 0;
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

    if (verbose) fprintf(stderr, "note: flushed all binaries from database\n");
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

    datapath_should_free = 1;

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
 * free_datapath()
 * frees the datapaths allocated by init_datapath() if necessary
 *
 * should be called at program end
 */
void free_datapath() {
    if (datapath_should_free) free(data_dir);
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
        if (verbose) fprintf(stderr, "note: using module root %s\n", cur_path);
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
    l->list[l->len - 1] = strdup(item);
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
    DIR* d;
    struct dirent* dp;
    struct stat st;

    /* try and walk the root */
    if (!(d = opendir(root))) {
        if (verbose) fprintf(stderr, "warning: couldn't open module root %s: %m\n", root);
        return -1;
    }

    while ((dp = readdir(d))) {
        if (!strcmp(dp->d_name, ".") || !strcmp(dp->d_name, "..")) continue;

        char* abs_path = join_path(root, dp->d_name);

        if (stat(abs_path, &st)) {
            fprintf(stderr, "warning: stat() failed for %s: %m\n", abs_path);
            free(abs_path);
            continue;
        }

        if (S_ISDIR(st.st_mode)) {
            build_module_dir(root, abs_path, dp->d_name);
        }

        free(abs_path);
    }

    closedir(d);
    return 0;
}

/*
 * build_module_dir(root, dir, name)
 * builds the cache for a module directory (one level below a root)
 *
 * root: module root directory (passed from build_root())
 * dir: module directory
 * name: module name
 *
 * returns nonzero if something goes wrong
 */
int build_module_dir(char* root, char* module_dir, char* name) {
    DIR* d;
    struct dirent* dp;
    struct stat st;

    /* try and walk the dir */
    if (!(d = opendir(module_dir))) {
        fprintf(stderr, "warning: couldn't open module dir %s: %m\n", module_dir);
        return -1;
    }

    while ((dp = readdir(d))) {
        char* abs_path = join_path(module_dir, dp->d_name);

        if (stat(abs_path, &st)) {
            fprintf(stderr, "warning: stat() failed for %s: %m\n", abs_path);
            free(abs_path);
            continue;
        }

        if (S_ISREG(st.st_mode) || S_ISLNK(st.st_mode)) {
            build_module_file(root, name, dp->d_name, abs_path);
        }

        free(abs_path);
    }

    closedir(d);
    return 0;
}

/*
 * build_module_file(root, name, file_name, file_path)
 * builds a single module file
 *
 * root: module root path
 * module_name: module directory basename
 * module_file_name: module file basename
 * module_file_path: full module file path
 *
 * returns nonzero if something goes wrong
 */
int build_module_file(char* root, char* module_name, char* module_file_name, char* module_file_path) {
    char* code;

    /* compute the module code */
    code = join_path(module_name, module_file_name);

    /* chop off .lua extensions */
    if (!strcmp(code + strlen(code) - 4, ".lua")) {
        code[strlen(code) - 4] = 0;
    }

    string_list paths = {0};

    if (verbose) fprintf(stderr, "note: building %s from %s\n", code, module_file_path);

    if (extract_tcl(module_file_path, &paths)) {
        extract_lmod(module_file_path, &paths);
    }

    if (verbose) fprintf(stderr, "note: searching %d potential paths for %s\n", paths.len, code);
    
    for (int i = 0; i < paths.len; ++i) {
        build_potential_path(root, code, paths.list[i]);
    }

    free(code);
    string_list_free(&paths);

    return 0;
}

/*
 * build_potential_path(root, code, path)
 * searches a potential PATH for binaries
 *
 * root: module root
 * code: module code
 * path: PATH value
 *
 * return nonzero if something goes wrong
 */
int build_potential_path(char* root, char* code, char* path) {
    DIR* d;
    struct dirent* dp;
    struct stat st;
    int res;

    /* try and walk the dir */
    if (!(d = opendir(path))) {
        if (verbose) fprintf(stderr, "warning: couldn't open potential path %s (from %s): %m\n", path, code);
        return -1;
    }

    while ((dp = readdir(d))) {
        char* abs_path = join_path(path, dp->d_name);

        if (stat(abs_path, &st)) {
            fprintf(stderr, "warning: stat() failed for %s: %m\n", abs_path);
            free(abs_path);
            continue;
        }

        if (S_ISREG(st.st_mode) || S_ISLNK(st.st_mode)) {
            /* check that we have execute permission */
            if (access(abs_path, X_OK)) continue;

            res = sqlite3_bind_text(stmt_add_bin, 1, root, strlen(root), SQLITE_TRANSIENT) ||
                  sqlite3_bind_text(stmt_add_bin, 2, code, strlen(code), SQLITE_TRANSIENT) ||
                  sqlite3_bind_text(stmt_add_bin, 3, dp->d_name, strlen(dp->d_name), SQLITE_TRANSIENT);

            /* OK, we can bind parameters and insert the binary into the db */
            if (res) {
                fprintf(stderr, "error: unexpected failure binding parameter to add_bin: %s\n", sqlite3_errmsg(db_connection));
                free(abs_path);
                continue;
            }

            if (sqlite3_step(stmt_add_bin) != SQLITE_DONE) {
                fprintf(stderr, "error: error executing add_bin statement: %s\n", sqlite3_errmsg(db_connection));
                free(abs_path);
                continue;
            }

            ++module_count;
            sqlite3_reset(stmt_add_bin);
        }

        free(abs_path);
    }

    closedir(d);
    return 0;
}

/*
 * extract_lmod(path, list)
 * extracts additional PATH variables from Lmod files
 *
 * path: path to module file
 * list: destination string list
 */
int extract_lmod(char* path, string_list* list) {
    FILE* f;
    char linebuf[LINEBUF_SIZE];
    int len, count;
    regmatch_t matches[3];

    if (!(f = fopen(path, "r"))) { 
        fprintf(stderr, "warning: couldn't open %s for reading: %m\n", path);
        return -1;
    }

    count = 0;
    while (fgets(linebuf, sizeof linebuf, f)) {
        /* strip newline */
        len = strlen(linebuf);
        if (linebuf[len - 1] == '\n') linebuf[len - 1] = 0;

        /* execute regex */
        if (!regexec(&reg_lmod, linebuf, 3, matches, 0)) {
            if (matches[2].rm_so < 0) continue;
            linebuf[matches[2].rm_eo] = 0;
            string_list_append(list, linebuf + matches[2].rm_so);
            ++count;
        }
    }

    fclose(f);

    if (verbose) fprintf(stderr, "note: extract_lmod() pulled %d paths from %s\n", count, path);
    return 0;
}

/*
 * extract_tcl(path, list)
 * extracts additonal PATH variables from Tcl modulefiles
 *
 * path: path to module file
 * list: destination string list
 */
int extract_tcl(char* path, string_list* list) {
    FILE* f;
    char linebuf[LINEBUF_SIZE];
    int count;

    if (!(f = fopen(path, "r"))) { 
        fprintf(stderr, "warning: couldn't open %s for reading: %m\n", path);
        return -1;
    }

    /* test that the first line contains the magic Tcl cookie */
    if (!fgets(linebuf, sizeof linebuf, f)) {
        fclose(f);
        return -1;
    }

    if (strncmp(linebuf, "#%Module", 8)) {
        fclose(f);
        return -1;
    }

    count = 0;
    while (fgets(linebuf, sizeof linebuf, f)) {
        if (*linebuf == '\n') continue;
        if (*linebuf == '#') continue;

        /* get command */
        char* cmd = strtok(linebuf, " \t");
        if (!cmd) continue;

        /* 'set' command, add a pair to the environment */
        if (!strcmp(cmd, "set")) {
            char* key = strtok(NULL, " \t");
            if (!key) continue;
            char* val = strtok(NULL, "\n");
            if (!val) continue;

            char* val_exp = expand_string(val);
            if (!val_exp) continue;

            /* 
             * we can just use our program environment to store variables.
             * this makes POSIX wordexp() incredibly useful here
             *
             * TODO: this might be better done differently, not sure why though
             */
            setenv(key, val_exp, 1);
            free(val_exp);
        }

        /* 'prepend-path' and 'append-path' */
        if (!strcmp(cmd, "prepend-path") || !strcmp(cmd, "append-path")) {
            char* key = strtok(NULL, " \t");
            if (!key || strcmp(key, "PATH")) continue;
            char* val = strtok(NULL, "\n");
            if (!val) continue;

            char* val_exp = expand_string(val);
            if (!val_exp) {
                if (verbose) fprintf(stderr, "warning: expansion failed in %s value: %s\n", cmd, val);
                continue;
            }

            /* seems ok, add the value to the potential paths */
            string_list_append(list, val_exp);
            free(val_exp);
            ++count;
        }
    }

    fclose(f);

    if (verbose) fprintf(stderr, "note: extract_tcl() pulled %d paths from %s\n", count, path);
    return 0;
}

/*
 * search_binary(bin)
 * searches the database for providers for a binary
 *
 * bin: command to search for
 */
search_result search_binary(char* bin) {
    search_result out;
    memset(&out, 0, sizeof out);
    int res;

    if (sqlite3_bind_text(stmt_search_bin_exact, 1, bin, strlen(bin), SQLITE_TRANSIENT)) {
        fprintf(stderr, "error: failed to bind parameter to search_bin_exact: %s\n", sqlite3_errmsg(db_connection));
        return out;
    }

    while (1) {
        res = sqlite3_step(stmt_search_bin_exact);

        if (res == SQLITE_ROW) {
            char* root = (char*) sqlite3_column_text(stmt_search_bin_exact, 0);
            char* code = (char*) sqlite3_column_text(stmt_search_bin_exact, 1);

            search_result_append(&out, root, code, bin);
        }

        if (res == SQLITE_DONE) {
            break;
        }
    }

    sqlite3_reset(stmt_search_bin_exact);
    return out;
}

/*
 * search_similar(bin)
 * searches the database for similar binary entries
 *
 * bin: command to search for
 */
search_result search_similar(char* bin) {
    search_result out;
    memset(&out, 0, sizeof out);
    int res;

    /* we need to add a '%' to the beginning and end of 'bin' to allow wildcard matching in SQL LIKE */
    int bin_len = strlen(bin);
    char* bin_param = malloc(bin_len + 3);
    snprintf(bin_param, bin_len + 3, "%%%s%%", bin);

    if (sqlite3_bind_text(stmt_search_bin_similar, 1, bin_param, -1, SQLITE_TRANSIENT)) {
        fprintf(stderr, "error: failed to bind parameter to search_bin_exact: %s\n", sqlite3_errmsg(db_connection));
        return out;
    }

    free(bin_param);

    while (1) {
        res = sqlite3_step(stmt_search_bin_similar);

        if (res == SQLITE_ROW) {
            char* root = (char*) sqlite3_column_text(stmt_search_bin_similar, 0);
            char* code = (char*) sqlite3_column_text(stmt_search_bin_similar, 1);
            char* bin = (char*) sqlite3_column_text(stmt_search_bin_similar, 2);

            search_result_append(&out, root, code, bin);
        }

        if (res == SQLITE_DONE) {
            break;
        }
    }

    sqlite3_reset(stmt_search_bin_similar);
    return out;
}

/*
 * init_regex()
 * initializes regular expressions
 *
 * must be called before any module parsing is done (via lmod)
 * returns nonzero on compilation failure
 */
int init_regex() {
    int res;
    char ebuf[LINEBUF_SIZE];

    if ((res = regcomp(&reg_lmod, reg_src_lmod, REG_EXTENDED | REG_NEWLINE))) {
        regerror(res, &reg_lmod, ebuf, sizeof ebuf);
        fprintf(stderr, "error: failed to compile lmod regex: %s\n", ebuf);
        return -1;
    }

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

/*
 * search_result_append(list, root, code, bin)
 * append an entry to a search result
 *
 * list: list to append to
 * root: module root
 * code: module code
 * bin:  provided binary
 */
void search_result_append(search_result* list, char* root, char* code, char* bin) {
    string_list_append(&list->roots, root);
    string_list_append(&list->codes, code);
    string_list_append(&list->bins, bin);
}

/*
 * search_result_free(list)
 * free entries from a search result
 *
 * list: results to free
 */
void search_result_free(search_result* list) {
    string_list_free(&list->roots);
    string_list_free(&list->codes);
    string_list_free(&list->bins);
}

/*
 * usage(a0)
 * prints usage information
 *
 * a0: argv[0]
 */
void usage(char* a0) {
    fprintf(stderr, "usage: %s [OPTIONS] <SUBCOMMAND>\n\n", a0);
    fprintf(stderr, "SUBCOMMAND:\n");
    fprintf(stderr, "\t%-16sshow this message\n", "help");
    fprintf(stderr, "\t%-16srebuild module cache\n", "build");
    fprintf(stderr, "\t%-16ssearch for exact providers\n", "search <name>");
    fprintf(stderr, "\t%-16ssearch for similar providers\n\n", "like <name>");
    fprintf(stderr, "OPTIONS:\n");
    fprintf(stderr, "\t%-16sdata directory (default: ~/.cache/lmc)\n", "-d <path>");
    fprintf(stderr, "\t%-16smodule path string (default: $MODULEPATH)\n", "-m <path>");
}

/*
 * expand_string(str)
 * performs string expansion and environment substitution
 *
 * str: string to parse
 *
 * returns an allocated string value or NULL if an error occurs
 */
char* expand_string(char* str) {
    wordexp_t exp;
    if (wordexp(str, &exp, 0)) {
        return NULL;
    }

    char* final_val = NULL;
    int final_len = 0;
    for (int i = 0; i < exp.we_wordc; ++i) {
        int wlen = strlen(exp.we_wordv[i]);
        final_val = realloc(final_val, final_len + wlen + 1);
        strncpy(final_val + final_len, exp.we_wordv[i], wlen);
        final_len += wlen;
        final_val[final_len] = 0;
    }

    wordfree(&exp);
    return final_val;
}

/*
 * free_regex()
 *
 * frees compiled regular expressions
 * should be called at end of program
 */
void free_regex() {
    regfree(&reg_lmod);
}

/*
 * free_modulepath()
 * frees the module paths allocated by init_modulepath()
 *
 * should be called at program end
 */
void free_modulepath() {
    string_list_free(&module_roots);
}
