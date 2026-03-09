// Modified from Luau CLI/src/Web.cpp
// Changes: captures print() output into a buffer and returns it,
// instead of writing to stdout.
#include "lua.h"
#include "lualib.h"
#include "luacode.h"

#include "Luau/Common.h"

#include <string>
#include <memory>
#include <string.h>

// Global output buffer for capturing print() calls
static std::string g_output;

// Custom print function that appends to g_output
static int customPrint(lua_State* L)
{
    int n = lua_gettop(L);
    for (int i = 1; i <= n; i++)
    {
        size_t len;
        const char* s = luaL_tolstring(L, i, &len);
        if (i > 1)
            g_output += "\t";
        g_output.append(s, len);
        lua_pop(L, 1);
    }
    g_output += "\n";
    return 0;
}

static void setupState(lua_State* L)
{
    luaL_openlibs(L);

    // Override print with our capturing version
    lua_pushcfunction(L, customPrint, "print");
    lua_setglobal(L, "print");

    luaL_sandbox(L);
}

static std::string runCode(lua_State* L, const std::string& source)
{
    size_t bytecodeSize = 0;
    char* bytecode = luau_compile(source.data(), source.length(), nullptr, &bytecodeSize);
    int result = luau_load(L, "=stdin", bytecode, bytecodeSize, 0);
    free(bytecode);

    if (result != 0)
    {
        size_t len;
        const char* msg = lua_tolstring(L, -1, &len);
        std::string error(msg, len);
        lua_pop(L, 1);
        return error;
    }

    lua_State* T = lua_newthread(L);

    lua_pushvalue(L, -2);
    lua_remove(L, -3);
    lua_xmove(L, T, 1);

    int status = lua_resume(T, NULL, 0);

    if (status == 0)
    {
        int n = lua_gettop(T);

        if (n)
        {
            luaL_checkstack(T, LUA_MINSTACK, "too many results to print");
            lua_getglobal(T, "print");
            lua_insert(T, 1);
            lua_pcall(T, n, 0, 0);
        }

        lua_pop(L, 1); // pop T
        return std::string();
    }
    else
    {
        std::string error;

        lua_Debug ar;
        if (lua_getinfo(L, 0, "sln", &ar))
        {
            error += ar.short_src;
            error += ':';
            error += std::to_string(ar.currentline);
            error += ": ";
        }

        if (status == LUA_YIELD)
        {
            error += "thread yielded unexpectedly";
        }
        else if (const char* str = lua_tostring(T, -1))
        {
            error += str;
        }

        error += "\nstack backtrace:\n";
        error += lua_debugtrace(T);

        lua_pop(L, 1); // pop T
        return error;
    }
}

extern "C" const char* executeScript(const char* source)
{
    // setup flags - enable all Luau feature flags
    for (Luau::FValue<bool>* flag = Luau::FValue<bool>::list; flag; flag = flag->next)
        if (strncmp(flag->name, "Luau", 4) == 0)
            flag->value = true;

    // Clear output buffer
    g_output.clear();

    // create new state
    std::unique_ptr<lua_State, void (*)(lua_State*)> globalState(luaL_newstate(), lua_close);
    lua_State* L = globalState.get();

    // setup state
    setupState(L);

    // sandbox thread
    luaL_sandboxthread(L);

    // static string for caching result
    static std::string result;

    // run code + collect error
    std::string error = runCode(L, source);

    if (!error.empty())
    {
        // Return error prefixed with "ERROR:" so JS can distinguish
        result = "ERROR:" + error;
    }
    else
    {
        // Return captured output
        result = g_output;
    }

    return result.c_str();
}
