$       ! Used by the main descrip.mms to print the installation complete
$       ! message.
$       ! Arguments:
$       ! P1    startup / setup / shutdown scripts directory
$       ! P2    distinguishing version number ("major version")
$
$       systartup = p1
$       osslver = p2
$
$       WRITE SYS$OUTPUT "Installation complete"
$       WRITE SYS$OUTPUT ""
$       WRITE SYS$OUTPUT "The following commands need to be executed to enable you to use OpenSSL:"
$       WRITE SYS$OUTPUT ""
$       WRITE SYS$OUTPUT "- to set up OpenSSL logical names:"
$       WRITE SYS$OUTPUT "  @''systartup'openssl_startup''osslver'"
$       WRITE SYS$OUTPUT ""
$       WRITE SYS$OUTPUT "- to define the OpenSSL command"
$       WRITE SYS$OUTPUT "  @''systartup'openssl_utils''osslver'"
$       WRITE SYS$OUTPUT ""
