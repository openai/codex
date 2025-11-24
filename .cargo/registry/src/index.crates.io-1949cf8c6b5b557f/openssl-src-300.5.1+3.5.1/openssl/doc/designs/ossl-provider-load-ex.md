OSSL_PROVIDER_load_ex - activating providers with run-time configuration
========================================================================

Currently any provider run-time activation requires the presence of the
initialization parameters in the OpenSSL configuration file. Otherwise the
provider will be activated with some default settings, that may or may not
work for a particular application. For real-world systems it may require
providing a specially designed OpenSSL configuration file and passing it somehow
(e.g. via environment), which has obvious drawbacks.

We need a possibility to initialize providers on per-application level
according to per-application parameters. It's necessary for example for PKCS#11
provider (where different applications may use different devices with different
drivers) and will be useful for some other providers. In case of Red Hat it is
also usable for FIPS provider.

OpenSSL 3.2 introduces the API

```C
OSSL_PROVIDER *OSSL_PROVIDER_load_ex(OSSL_LIB_CTX *libctx, const char *name,
                                     OSSL_PARAM params[]);
```

intended to configure the provider at load time.

It accepts only parameters of type `OSSL_PARAM_UTF8_STRING` because any
provider can be initialized from the config file where the values are
represented as strings and provider init function has to deal with it.

Explicitly configured parameters can differ from the parameters named in the
configuration file. Here are the current design decisions and some possible
future steps.

Real-world cases
----------------

Many applications use PKCS#11 API with specific drivers. OpenSSL PKCS#11
provider <https://github.com/latchset/pkcs11-provider> also provides a set of
tweaks usable in particular situations. So there are several scenarios for which
the new API can be used:

1. Configure a provider in the config file, activate on demand
2. Load/activate a provider run-time with parameters

Current design
--------------

When the provider is already loaded an activated in the current library context,
the `OSSL_PROVIDER_load_ex` call simply returns the active provider and the
extra parameters are ignored.

In all other cases, the extra parameters provided by the `OSSL_PROVIDER_load_ex`
call are applied and the values from the config file are ignored.

Separate instances of the provider can be loaded in the separate library
contexts.

Several instances of the same provider can be loaded in the same context using
different section names, module names (e.g. via symlinks) and provider names.
But unless the provider supports some configuration options, the algorithms in
this case will have the same `provider` property and the result of fetching is
not determined. We strongly discourage against this trick.

Changing the loaded provider configuration at runtime is not supported. If
it is necessary, the provider needs to be unloaded using `OSSL_PROVIDER_unload`
and reloaded using `OSSL_PROVIDER_load` or `OSSL_PROVIDER_load_ex` should be used.

Possible future steps
---------------------

1. We should provide some API function accessing the configuration parameters
   of a particular provider. Having it, the application will be able to combine
   some default values with the app-specific ones in more or less intellectual
   way.

2. We probably should remove the `INFOPAIR` structure and use the `OSSL_PARAM`
   one instead.
