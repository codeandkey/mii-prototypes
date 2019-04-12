# lmc caching engine

# use c99 standard
set (CMAKE_C_FLAGS "-std=c99")

add_executable (lmc lmc.c)
include_directories (${SQLITE3_INCLUDE_DIR})
target_link_libraries (lmc ${SQLITE3_LIBRARIES})

install (TARGETS lmc DESTINATION bin)
