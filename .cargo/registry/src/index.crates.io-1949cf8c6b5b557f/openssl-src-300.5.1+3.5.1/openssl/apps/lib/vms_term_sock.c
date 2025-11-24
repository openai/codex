/*
 * Copyright 2016-2022 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright 2016 VMS Software, Inc. All Rights Reserved.
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

#ifdef __VMS
# define OPENSSL_SYS_VMS
# pragma message disable DOLLARID


# include <openssl/opensslconf.h>

# if !defined(_POSIX_C_SOURCE) && defined(OPENSSL_SYS_VMS)
/*
 * On VMS, you need to define this to get the declaration of fileno().  The
 * value 2 is to make sure no function defined in POSIX-2 is left undefined.
 */
#  define _POSIX_C_SOURCE 2
# endif

# include <stdio.h>

# undef _POSIX_C_SOURCE

# include <sys/types.h>
# include <sys/socket.h>
# include <netinet/in.h>
# include <inet.h>
# include <unistd.h>
# include <string.h>
# include <errno.h>
# include <starlet.h>
# include <iodef.h>
# ifdef __alpha
#  include <iosbdef.h>
# else
typedef struct _iosb {           /* Copied from IOSBDEF.H for Alpha  */
#  pragma __nomember_alignment
    __union  {
        __struct  {
            unsigned short int iosb$w_status; /* Final I/O status           */
            __union  {
                __struct  {             /* 16-bit byte count variant        */
                    unsigned short int iosb$w_bcnt; /* 16-bit byte count    */
                    __union  {
                        unsigned int iosb$l_dev_depend; /* 32-bit device dependent info */
                        unsigned int iosb$l_pid; /* 32-bit pid              */
                    } iosb$r_l;
                } iosb$r_bcnt_16;
                __struct  {             /* 32-bit byte count variant        */
                    unsigned int iosb$l_bcnt; /* 32-bit byte count (unaligned) */
                    unsigned short int iosb$w_dev_depend_high; /* 16-bit device dependent info */
                } iosb$r_bcnt_32;
            } iosb$r_devdepend;
        } iosb$r_io_64;
        __struct  {
            __union  {
                unsigned int iosb$l_getxxi_status; /* Final GETxxI status   */
                unsigned int iosb$l_reg_status; /* Final $Registry status   */
            } iosb$r_l_status;
            unsigned int iosb$l_reserved; /* Reserved field                 */
        } iosb$r_get_64;
    } iosb$r_io_get;
} IOSB;

#  if !defined(__VAXC)
#   define iosb$w_status iosb$r_io_get.iosb$r_io_64.iosb$w_status
#   define iosb$w_bcnt iosb$r_io_get.iosb$r_io_64.iosb$r_devdepend.iosb$r_bcnt_16.iosb$w_bcnt
#   define iosb$r_l        iosb$r_io_get.iosb$r_io_64.iosb$r_devdepend.iosb$r_bcnt_16.iosb$r_l
#   define iosb$l_dev_depend iosb$r_l.iosb$l_dev_depend
#   define iosb$l_pid iosb$r_l.iosb$l_pid
#   define iosb$l_bcnt iosb$r_io_get.iosb$r_io_64.iosb$r_devdepend.iosb$r_bcnt_32.iosb$l_bcnt
#   define iosb$w_dev_depend_high iosb$r_io_get.iosb$r_io_64.iosb$r_devdepend.iosb$r_bcnt_32.iosb$w_dev_depend_high
#   define iosb$l_getxxi_status iosb$r_io_get.iosb$r_get_64.iosb$r_l_status.iosb$l_getxxi_status
#   define iosb$l_reg_status iosb$r_io_get.iosb$r_get_64.iosb$r_l_status.iosb$l_reg_status
#  endif          /* #if !defined(__VAXC) */

# endif  /* End of IOSBDEF */

# include <efndef.h>
# include <stdlib.h>
# include <ssdef.h>
# include <time.h>
# include <stdarg.h>
# include <descrip.h>

# include "vms_term_sock.h"

# ifdef __alpha
static struct _iosb TerminalDeviceIosb;
# else
IOSB TerminalDeviceIosb;
# endif

static char TerminalDeviceBuff[255 + 2];
static int TerminalSocketPair[2] = {0, 0};
static unsigned short TerminalDeviceChan = 0;

static int CreateSocketPair (int, int, int, int *);
static void SocketPairTimeoutAst (int);
static int TerminalDeviceAst (int);
static void LogMessage (char *, ...);

/*
** Socket Pair Timeout Value (must be 0-59 seconds)
*/
# define SOCKET_PAIR_TIMEOUT_VALUE 20

/*
** Socket Pair Timeout Block which is passed to timeout AST
*/
typedef struct _SocketPairTimeoutBlock {
    unsigned short SockChan1;
    unsigned short SockChan2;
} SPTB;

# ifdef TERM_SOCK_TEST

/*----------------------------------------------------------------------------*/
/*                                                                            */
/*----------------------------------------------------------------------------*/
int main (int argc, char *argv[], char *envp[])
{
    char TermBuff[80];
    int TermSock,
        status,
        len;

    LogMessage ("Enter 'q' or 'Q' to quit ...");
    while (OPENSSL_strcasecmp (TermBuff, "Q")) {
        /*
        ** Create the terminal socket
        */
        status = TerminalSocket (TERM_SOCK_CREATE, &TermSock);
        if (status != TERM_SOCK_SUCCESS)
            exit (1);

        /*
        ** Process the terminal input
        */
        LogMessage ("Waiting on terminal I/O ...\n");
        len = recv (TermSock, TermBuff, sizeof(TermBuff), 0) ;
        TermBuff[len] = '\0';
        LogMessage ("Received terminal I/O [%s]", TermBuff);

        /*
        ** Delete the terminal socket
        */
        status = TerminalSocket (TERM_SOCK_DELETE, &TermSock);
        if (status != TERM_SOCK_SUCCESS)
            exit (1);
    }

    return 1;

}
# endif

/*----------------------------------------------------------------------------*/
/*                                                                            */
/*----------------------------------------------------------------------------*/
int TerminalSocket (int FunctionCode, int *ReturnSocket)
{
    int status;
    $DESCRIPTOR (TerminalDeviceDesc, "SYS$COMMAND");

    /*
    ** Process the requested function code
    */
    switch (FunctionCode) {
    case TERM_SOCK_CREATE:
        /*
        ** Create a socket pair
        */
        status = CreateSocketPair (AF_INET, SOCK_STREAM, 0, TerminalSocketPair);
        if (status == -1) {
            LogMessage ("TerminalSocket: CreateSocketPair () - %08X", status);
            if (TerminalSocketPair[0])
                close (TerminalSocketPair[0]);
            if (TerminalSocketPair[1])
                close (TerminalSocketPair[1]);
            return TERM_SOCK_FAILURE;
        }

        /*
        ** Assign a channel to the terminal device
        */
        status = sys$assign (&TerminalDeviceDesc,
                             &TerminalDeviceChan,
                             0, 0, 0);
        if (! (status & 1)) {
            LogMessage ("TerminalSocket: SYS$ASSIGN () - %08X", status);
            close (TerminalSocketPair[0]);
            close (TerminalSocketPair[1]);
            return TERM_SOCK_FAILURE;
        }

        /*
        ** Queue an async IO to the terminal device
        */
        status = sys$qio (EFN$C_ENF,
                          TerminalDeviceChan,
                          IO$_READVBLK,
                          &TerminalDeviceIosb,
                          TerminalDeviceAst,
                          0,
                          TerminalDeviceBuff,
                          sizeof(TerminalDeviceBuff) - 2,
                          0, 0, 0, 0);
        if (! (status & 1)) {
            LogMessage ("TerminalSocket: SYS$QIO () - %08X", status);
            close (TerminalSocketPair[0]);
            close (TerminalSocketPair[1]);
            return TERM_SOCK_FAILURE;
        }

        /*
        ** Return the input side of the socket pair
        */
        *ReturnSocket = TerminalSocketPair[1];
        break;

    case TERM_SOCK_DELETE:
        /*
        ** Cancel any pending IO on the terminal channel
        */
        status = sys$cancel (TerminalDeviceChan);
        if (! (status & 1)) {
            LogMessage ("TerminalSocket: SYS$CANCEL () - %08X", status);
            close (TerminalSocketPair[0]);
            close (TerminalSocketPair[1]);
            return TERM_SOCK_FAILURE;
        }

        /*
        ** Deassign the terminal channel
        */
        status = sys$dassgn (TerminalDeviceChan);
        if (! (status & 1)) {
            LogMessage ("TerminalSocket: SYS$DASSGN () - %08X", status);
            close (TerminalSocketPair[0]);
            close (TerminalSocketPair[1]);
            return TERM_SOCK_FAILURE;
        }

        /*
        ** Close the terminal socket pair
        */
        close (TerminalSocketPair[0]);
        close (TerminalSocketPair[1]);

        /*
        ** Return the initialized socket
        */
        *ReturnSocket = 0;
        break;

    default:
        /*
        ** Invalid function code
        */
        LogMessage ("TerminalSocket: Invalid Function Code - %d", FunctionCode);
        return TERM_SOCK_FAILURE;
        break;
    }

    /*
    ** Return success
    */
    return TERM_SOCK_SUCCESS;

}

/*----------------------------------------------------------------------------*/
/*                                                                            */
/*----------------------------------------------------------------------------*/
static int CreateSocketPair (int SocketFamily,
                             int SocketType,
                             int SocketProtocol,
                             int *SocketPair)
{
    struct dsc$descriptor AscTimeDesc = {0, DSC$K_DTYPE_T, DSC$K_CLASS_S, NULL};
    static const char* LocalHostAddr = {"127.0.0.1"};
    unsigned short TcpAcceptChan = 0,
        TcpDeviceChan = 0;
    unsigned long BinTimeBuff[2];
    struct sockaddr_in sin;
    char AscTimeBuff[32];
    short LocalHostPort;
    int status;
    unsigned int slen;

# ifdef __alpha
    struct _iosb iosb;
# else
    IOSB iosb;
# endif

    int SockDesc1 = 0,
        SockDesc2 = 0;
    SPTB sptb;
    $DESCRIPTOR (TcpDeviceDesc, "TCPIP$DEVICE");

    /*
    ** Create a socket
    */
    SockDesc1 = socket (SocketFamily, SocketType, 0);
    if (SockDesc1 < 0) {
        LogMessage ("CreateSocketPair: socket () - %d", errno);
        return -1;
    }

    /*
    ** Initialize the socket information
    */
    slen = sizeof(sin);
    memset ((char *) &sin, 0, slen);
    sin.sin_family = SocketFamily;
    sin.sin_addr.s_addr = inet_addr (LocalHostAddr);
    sin.sin_port = 0;

    /*
    ** Bind the socket to the local IP
    */
    status = bind (SockDesc1, (struct sockaddr *) &sin, slen);
    if (status < 0) {
        LogMessage ("CreateSocketPair: bind () - %d", errno);
        close (SockDesc1);
        return -1;
    }

    /*
    ** Get the socket name so we can save the port number
    */
    status = getsockname (SockDesc1, (struct sockaddr *) &sin, &slen);
    if (status < 0) {
        LogMessage ("CreateSocketPair: getsockname () - %d", errno);
        close (SockDesc1);
        return -1;
    } else
        LocalHostPort = sin.sin_port;

    /*
    ** Setup a listen for the socket
    */
    listen (SockDesc1, 5);

    /*
    ** Get the binary (64-bit) time of the specified timeout value
    */
    BIO_snprintf(AscTimeBuff, sizeof(AscTimeBuff), "0 0:0:%02d.00", SOCKET_PAIR_TIMEOUT_VALUE);
    AscTimeDesc.dsc$w_length = strlen (AscTimeBuff);
    AscTimeDesc.dsc$a_pointer = AscTimeBuff;
    status = sys$bintim (&AscTimeDesc, BinTimeBuff);
    if (! (status & 1)) {
        LogMessage ("CreateSocketPair: SYS$BINTIM () - %08X", status);
        close (SockDesc1);
        return -1;
    }

    /*
    ** Assign another channel to the TCP/IP device for the accept.
    ** This is the channel that ends up being connected to.
    */
    status = sys$assign (&TcpDeviceDesc, &TcpDeviceChan, 0, 0, 0);
    if (! (status & 1)) {
        LogMessage ("CreateSocketPair: SYS$ASSIGN () - %08X", status);
        close (SockDesc1);
        return -1;
    }

    /*
    ** Get the channel of the first socket for the accept
    */
    TcpAcceptChan = decc$get_sdc (SockDesc1);

    /*
    ** Perform the accept using $QIO so we can do this asynchronously
    */
    status = sys$qio (EFN$C_ENF,
                      TcpAcceptChan,
                      IO$_ACCESS | IO$M_ACCEPT,
                      &iosb,
                      0, 0, 0, 0, 0,
                      &TcpDeviceChan,
                      0, 0);
    if (! (status & 1)) {
        LogMessage ("CreateSocketPair: SYS$QIO () - %08X", status);
        close (SockDesc1);
        sys$dassgn (TcpDeviceChan);
        return -1;
    }

    /*
    ** Create the second socket to do the connect
    */
    SockDesc2 = socket (SocketFamily, SocketType, 0);
    if (SockDesc2 < 0) {
        LogMessage ("CreateSocketPair: socket () - %d", errno);
        sys$cancel (TcpAcceptChan);
        close (SockDesc1);
        sys$dassgn (TcpDeviceChan);
        return (-1) ;
    }

    /*
    ** Setup the Socket Pair Timeout Block
    */
    sptb.SockChan1 = TcpAcceptChan;
    sptb.SockChan2 = decc$get_sdc (SockDesc2);

    /*
    ** Before we block on the connect, set a timer that can cancel I/O on our
    ** two sockets if it never connects.
    */
    status = sys$setimr (EFN$C_ENF,
                         BinTimeBuff,
                         SocketPairTimeoutAst,
                         &sptb,
                         0);
    if (! (status & 1)) {
        LogMessage ("CreateSocketPair: SYS$SETIMR () - %08X", status);
        sys$cancel (TcpAcceptChan);
        close (SockDesc1);
        close (SockDesc2);
        sys$dassgn (TcpDeviceChan);
        return -1;
    }

    /*
    ** Now issue the connect
    */
    memset ((char *) &sin, 0, sizeof(sin)) ;
    sin.sin_family = SocketFamily;
    sin.sin_addr.s_addr = inet_addr (LocalHostAddr) ;
    sin.sin_port = LocalHostPort ;

    status = connect (SockDesc2, (struct sockaddr *) &sin, sizeof(sin));
    if (status < 0) {
        LogMessage ("CreateSocketPair: connect () - %d", errno);
        sys$cantim (&sptb, 0);
        sys$cancel (TcpAcceptChan);
        close (SockDesc1);
        close (SockDesc2);
        sys$dassgn (TcpDeviceChan);
        return -1;
    }

    /*
    ** Wait for the asynch $QIO to finish.  Note that if the I/O was aborted
    ** (SS$_ABORT), then we probably canceled it from the AST routine - so log
    ** a timeout.
    */
    status = sys$synch (EFN$C_ENF, &iosb);
    if (! (iosb.iosb$w_status & 1)) {
        if (iosb.iosb$w_status == SS$_ABORT)
            LogMessage ("CreateSocketPair: SYS$QIO(iosb) timeout");
        else {
            LogMessage ("CreateSocketPair: SYS$QIO(iosb) - %d",
                        iosb.iosb$w_status);
            sys$cantim (&sptb, 0);
        }
        close (SockDesc1);
        close (SockDesc2);
        sys$dassgn (TcpDeviceChan);
        return -1;
    }

    /*
    ** Here we're successfully connected, so cancel the timer, convert the
    ** I/O channel to a socket fd, close the listener socket and return the
    ** connected pair.
    */
    sys$cantim (&sptb, 0);

    close (SockDesc1) ;
    SocketPair[0] = SockDesc2 ;
    SocketPair[1] = socket_fd (TcpDeviceChan);

    return (0) ;

}

/*----------------------------------------------------------------------------*/
/*                                                                            */
/*----------------------------------------------------------------------------*/
static void SocketPairTimeoutAst (int astparm)
{
    SPTB *sptb = (SPTB *) astparm;

    sys$cancel (sptb->SockChan2); /* Cancel the connect() */
    sys$cancel (sptb->SockChan1); /* Cancel the accept() */

    return;

}

/*----------------------------------------------------------------------------*/
/*                                                                            */
/*----------------------------------------------------------------------------*/
static int TerminalDeviceAst (int astparm)
{
    int status;

    /*
    ** Terminate the terminal buffer
    */
    TerminalDeviceBuff[TerminalDeviceIosb.iosb$w_bcnt] = '\0';
    strcat (TerminalDeviceBuff, "\n");

    /*
    ** Send the data read from the terminal device through the socket pair
    */
    send (TerminalSocketPair[0], TerminalDeviceBuff,
          TerminalDeviceIosb.iosb$w_bcnt + 1, 0);

    /*
    ** Queue another async IO to the terminal device
    */
    status = sys$qio (EFN$C_ENF,
                      TerminalDeviceChan,
                      IO$_READVBLK,
                      &TerminalDeviceIosb,
                      TerminalDeviceAst,
                      0,
                      TerminalDeviceBuff,
                      sizeof(TerminalDeviceBuff) - 1,
                      0, 0, 0, 0);

    /*
    ** Return status
    */
    return status;

}

/*----------------------------------------------------------------------------*/
/*                                                                            */
/*----------------------------------------------------------------------------*/
static void LogMessage (char *msg, ...)
{
    char *Month[] = {"Jan", "Feb", "Mar", "Apr", "May", "Jun",
                     "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"};
    static unsigned int pid = 0;
    va_list args;
    time_t CurTime;
    struct tm *LocTime;
    char MsgBuff[256];

    /*
    ** Get the process pid
    */
    if (pid == 0)
        pid = getpid ();

    /*
    ** Convert the current time into local time
    */
    CurTime = time (NULL);
    LocTime = localtime (&CurTime);

    /*
    ** Format the message buffer
    */
    BIO_snprintf(MsgBuff, sizeof(MsgBuff), "%02d-%s-%04d %02d:%02d:%02d [%08X] %s\n",
                 LocTime->tm_mday, Month[LocTime->tm_mon],
                 (LocTime->tm_year + 1900), LocTime->tm_hour, LocTime->tm_min,
                 LocTime->tm_sec, pid, msg);

    /*
    ** Get any variable arguments and add them to the print of the message
    ** buffer
    */
    va_start (args, msg);
    vfprintf (stderr, MsgBuff, args);
    va_end (args);

    /*
    ** Flush standard error output
    */
    fsync (fileno (stderr));

    return;

}
#endif
