# Fail if one of the public headers in the list HEADERS names Gecode, in any
# case: they must stand without it.
foreach(header IN LISTS HEADERS)
  file(STRINGS "${header}" lines REGEX "[Gg][Ee][Cc][Oo][Dd][Ee]")
  if(lines)
    list(GET lines 0 line)
    message(SEND_ERROR "${header} names Gecode: ${line}")
  endif()
endforeach()
