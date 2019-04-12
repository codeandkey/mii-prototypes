project (lmc)
cmake_minimum_required (VERSION 2.6)

# FindSQLite3 helper
set (CMAKE_MODULE_PATH ${CMAKE_MODULE_PATH} "${CMAKE_CURRENT_SOURCE_DIR}/cmake")
find_package (SQLite3 REQUIRED)

# lmc engine
add_subdirectory (lmc)

# install target for init scripts
install (DIRECTORY init DESTINATION share/lmc USE_SOURCE_PERMISSIONS)
