# CSV header names are not cleaned

The reader accepts empty and repeated column names as they are. Tools that look up a column by name
will not be able to tell two columns apart. Fix when the Clean profiler arrives: name empty headers
"Column N" and add " (2)", " (3)" to repeats. Not done now because it changes what other teams see.
