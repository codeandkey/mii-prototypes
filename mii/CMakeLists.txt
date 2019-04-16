# mii caching engine

# use c99 standard
set (CMAKE_C_FLAGS "-std=c99")

add_executable (mii mii.c)
include_directories (${SQLITE3_INCLUDE_DIR})
target_link_libraries (mii ${SQLITE3_LIBRARIES})

install (TARGETS mii DESTINATION bin)
