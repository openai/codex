#include <stdio.h>
#include <openssl/ec.h>
#include <openssl/evp.h>
#include <openssl/err.h>

int main(int argc, char *argv[])
{
    EVP_PKEY_CTX *pctx = NULL;

    pctx = EVP_PKEY_CTX_new_from_name(NULL, "NO_SUCH_ALGORITHM", NULL);
    EVP_PKEY_CTX_free(pctx);

    return 0;
}
