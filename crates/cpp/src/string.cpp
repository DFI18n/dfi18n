#include <string>
#include <cstring>

using namespace std;

extern "C"
{
  // Create a new empty C++ string
  void *cpp_create_string()
  {
    string *s = new string();
    return s;
  }

  // Create a new C++ string from a C-style string
  void *cpp_create_string_from_cstr(const char *cstr)
  {
    string *s = new string(cstr);
    return s;
  }

  // Create a new C++ string filled with 'ch' repeated 'len' times
  void *cpp_fill_string(char ch, size_t len)
  {
    string *s = new string(len, ch);
    return s;
  }

  // Delete a C++ string
  void cpp_delete_string(void *ptr)
  {
    string *s = (string *)ptr;
    delete s;
  }

  // Assign a C string to an existing C++ string (frees old buffer, copies new)
  void cpp_assign_string(void *target, const char *cstr)
  {
    string *s = (string *)target;
    s->assign(cstr);
  }
}
