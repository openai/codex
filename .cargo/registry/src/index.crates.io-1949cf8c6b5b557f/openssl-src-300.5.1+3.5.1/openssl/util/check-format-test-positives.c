/*
 * Copyright 2007-2025 The OpenSSL Project Authors. All Rights Reserved.
 * Copyright Siemens AG 2015-2022
 *
 * Licensed under the Apache License 2.0 (the "License").  You may not use
 * this file except in compliance with the License.  You can obtain a copy
 * in the file LICENSE in the source distribution or at
 * https://www.openssl.org/source/license.html
 */

/*
 * This demonstrates/tests cases where check-format.pl should report issues.
 * Some of the reports are due to sanity checks for proper nesting of comment
 * delimiters and parenthesis-like symbols, e.g., on unexpected/unclosed braces.
 */

/*
 * The '@'s after leading '*' in comment lines are used for self-tests:
 * they mark lines containing a single issue that should be reported.
 * Normally it should be reported while handling the given line,
 * but in case of delayed checks there is a following digit
 * indicating the number of reports expected for this line.
 */

/* this line is between 81 and 100 chars long, to be reported with -strict-len */

/* For each of the following set of lines the tool should complain once */
/*@ tab character: 	 */
/*@ intra-line carriage return character:  */
/*@ non-printable ASCII character:  */
/*@ non-ASCII character: ä */
/*@ whitespace at EOL: */ 
// /*@ end-of-line comment style not allowed (for C90 compatibility) */
 /*@0 intra-line comment indent off by 1, reported unless sloppy-cmt */
/*X */ /*@2 missing spc or '*' after comment start reported unless sloppy-spc */
/* X*/ /*@ missing space before comment end , reported unless sloppy-spc */
/*@ comment starting delimiter: /* inside intra-line comment */
 /*@0
  *@ above multi-line comment start indent off by 1, reported unless sloppy-cmt; this comment line is too long
   *@ multi-line comment indent further off by 1 relative to comment start
  *@ multi-line comment ending with text on last line */
/*@2 multi-line comment starting with text on first line
 *@ comment starting delimiter: /* inside multi-line comment
*@ multi-line comment indent off by -1
 *X*@ no spc after leading '*' in multi-line comment, reported unless sloppy-spc
 *@0 more than two spaces after .   in comment, no more reported
 *@0 more than two spaces after ?   in comment, no more reported
 *@0 more than two spaces after !   in comment, no more reported
*/ /*@ multi-line comment end indent off by -1 (relative to comment start) */
*/ /*@ unexpected comment ending delimiter outside comment */
/*- '-' for formatted comment not allowed in intra-line comment */
/*@ comment line is toooooooooooo wide by 1 char, or by 21 chars in case strict-len option is used */
#if ~0              /*@ '#if' with constant condition */
 #endif             /*@ indent of preproc. directive off by 1 (must be 0) */
#define X (1 +  1)  /*@0 extra space in body, reported unless sloppy-spc */
#define Y   1       /*@ extra space before body, reported unless sloppy-spc */ \
#define Z           /*@2 preprocessor directive within multi-line directive */
typedef struct  {   /*@0 extra space in code, reported unless sloppy-spc */
    enum {          /*@1 extra space  in intra-line comment, no more reported */
           w = 0 /*@ hanging expr indent off by 1, or 3 for lines after '{' */
             && 1,  /*@ hanging expr indent off by 3, or -1 for leading '&&' */
         x = 1,     /*@ hanging expr indent off by -1 */
          y,z       /*@ no space after ',', reported unless sloppy-spc */
    } e_member ;    /*@ space before ';', reported unless sloppy-spc */
    int v[1;        /*@ unclosed bracket in type declaration */
   union {          /*@ statement/type declaration indent off by -1 */
        struct{} s; /*@ no space before '{', reported unless sloppy-spc */
    }u_member;      /*@ no space after '}', reported unless sloppy-spc */
    } s_type;       /*@ statement/type declaration indent off by 4 */
int* somefunc();    /*@ no space before '*' in type decl, r unless sloppy-spc */
void main(int n) {  /*@ opening brace at end of function definition header */
    for (; ; ) ;    /*@ space before ')', reported unless sloppy-spc */
    for ( ; x; y) ; /*@2 space after '(' and before ';', unless sloppy-spc */
    for (;;n++) {   /*@ missing space after ';', reported unless sloppy-spc */
        return;     /*@0 (1-line) single statement in braces */
    }}              /*@2 code after '}' outside expr */
}                   /*@ unexpected closing brace (too many '}') outside expr */
)                   /*@ unexpected closing paren outside expr */
#endif              /*@ unexpected #endif */
int f (int a,       /*@ space after fn before '(', reported unless sloppy-spc */
      int b,        /*@ hanging expr indent off by -1 */
       long I)      /*@ single-letter name 'I' */
{ int x;            /*@ code after '{' opening a block */
    int xx = 1) +   /*@ unexpected closing parenthesis */
        0L <        /*@ constant on LHS of comparison operator */
        a] -        /*@ unexpected closing bracket */
        3: *        /*@ unexpected ':' (without preceding '?') within expr */
        4};         /*@ unexpected closing brace within expression */
    char y[] = {    /*@0 unclosed brace within initializer/enum expression */
        1* 1,       /*@ no space etc. before '*', reported unless sloppy-spc */
         2,         /*@ hanging expr indent (for lines after '{') off by 1 */
        (xx         /*@0 unclosed parenthesis in expression */
         ? y        /*@0 unclosed '? (conditional expression) */
         [0;        /*@4 unclosed bracket in expression */
    /*@ blank line within local decls */
   s_type s;        /*@2 local variable declaration indent off by -1 */
   t_type t;        /*@ local variable declaration indent again off by -1 */
    /* */           /*@0 missing blank line after local decls */
   somefunc(a,      /*@2 statement indent off by -1 */
          "aligned" /*@ expr indent off by -2 accepted if sloppy-hang */ "right"
           , b,     /*@ expr indent off by -1 */
           b,       /*@ expr indent as on line above, accepted if sloppy-hang */
    b, /*@ expr indent off -8 but @ extra indent accepted if sloppy-hang */
   "again aligned" /*@ expr indent off by -9 (left of stmt indent, */ "right",
            abc == /*@ .. so reported also with sloppy-hang; this line is too long by 6 or 26 chars */ 456
# define MAC(A) (A) /*@ nesting indent of preprocessor directive off by 1 */
             ? 1    /*@ hanging expr indent off by 1 */
              : 2); /*@ hanging expr indent off by 2, or 1 for leading ':' */
    if(a            /*@ missing space after 'if', reported unless sloppy-spc */
          /*@0 intra-line comment indent off by -1 (not: by 3 due to '&&') */
           && ! 0   /*@2 space after '!', reported unless sloppy-spc */
         || b ==    /*@ hanging expr indent off by 2, or -2 for leading '||' */
       (x<<= 1) +   /*@ missing space before '<<=' reported unless sloppy-spc */
       (xx+= 2) +   /*@ missing space before '+=', reported unless sloppy-spc */
       (a^ 1) +     /*@ missing space before '^', reported unless sloppy-spc */
       (y *=z) +    /*@ missing space after '*=' reported unless sloppy-spc */
       a %2 /       /*@ missing space after '%', reported unless sloppy-spc */
       1 +/* */     /*@ no space before comment, reported unless sloppy-spc */
       /* */+       /*@ no space after comment, reported unless sloppy-spc */
       s. e_member) /*@ space after '.', reported unless sloppy-spc */
         xx = a + b /*@ extra single-statement indent off by 1 */
               + 0; /*@ two times extra single-statement indent off by 3 */
    if (a ++)       /*@ space before postfix '++', reported unless sloppy-spc */
    {               /*@ {' not on same line as preceding 'if' */
        c;          /*@0 single stmt in braces, reported on 1-stmt */
    } else          /*@ missing '{' on same line after '} else' */
      {             /*@ statement indent off by 2 */
        d;          /*@0 single stmt in braces, reported on 1-stmt */
          }         /*@ statement indent off by 6 */
    if (1) f(a,     /*@ (non-brace) code after end of 'if' condition */
             b); else /*@ (non-brace) code before 'else' */
        do f(c, c); /*@ (non-brace) code after 'do' */
        while ( 2); /*@ space after '(', reported unless sloppy-spc */
    b; c;           /*@ more than one statement per line */
  outer:            /*@ outer label special indent off by 1 */
    do{             /*@ missing space before '{', reported unless sloppy-spc */
     inner:         /*@ inner label normal indent off by 1 */
        f (3,       /*@ space after fn before '(', reported unless sloppy-spc */
           4);      /*@0 false negative: should report single stmt in braces */
    }               /*@0 'while' not on same line as preceding '}' */
    while (a+ 0);   /*@2 missing space before '+', reported unless sloppy-spc */
    switch (b ) {   /*@ space before ')', reported unless sloppy-spc */
   case 1:          /*@ 'case' special statement indent off by -1 */
    case(2):       /*@ missing space after 'case', reported unless sloppy-spc */
    default: ;      /*@ code after 'default:' */
}                   /*@ statement indent off by -4 */
    return(      /*@ missing space after 'return', reported unless sloppy-spc */
           x); }    /*@ code before block-level '}' */
/* Here the tool should stop complaining apart from the below issues at EOF */

void f_looong_body()
{
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;
    ;


    ;               /*@ 2 essentially blank lines before, if !sloppy-spc */
}                   /*@ function body length > 200 lines */
#if X               /*@0 unclosed #if */
struct t {          /*@0 unclosed brace at decl/block level */
    enum {          /*@0 unclosed brace at enum/expression level */
          v = (1    /*@0 unclosed parenthesis */
               etyp /*@0 blank line follows just before EOF, if !sloppy-spc: */

