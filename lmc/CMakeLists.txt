# lmc caching engine

add_executable (lmc lmc.c)
include_directories (${SQLITE3_INCLUDE_DIR})
target_link_libraries (lmc ${SQLITE3_LIBRARIES})

install (TARGETS lmc DESTINATION bin)
