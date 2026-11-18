// Declarations only: the host config test never reads an Android property.
#pragma once
#define PROP_VALUE_MAX 92
extern "C" int __system_property_get(const char*, char*);