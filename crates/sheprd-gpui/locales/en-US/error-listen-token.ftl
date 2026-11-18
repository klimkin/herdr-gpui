# Why `sheprd remote relays token` could not read a private relay's listen
# token from standard input. The token itself is never shown.

error-listen-token-empty = No listen token on standard input. Pipe it in, or pass --clear to forget it.
error-listen-token-read = Could not read the listen token from standard input.
error-listen-token-terminal = Standard input is a terminal. Pipe the listen token in so it is not shown.
error-listen-token-text = The listen token on standard input is not text.
error-listen-token-token = A relay's listen token is 16 to 255 characters, without spaces.
error-listen-token-too-long = Standard input holds more than 1024 bytes; a listen token is at most 255.
