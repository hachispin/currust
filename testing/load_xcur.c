/// Testing file for Xcursors.

#include <X11/Xcursor/Xcursor.h>
#include <X11/Xlib.h>
#include <stdio.h>

int main() {
    Display *dpy = XOpenDisplay(NULL);

    if (!dpy) {
        puts("`XOpenDisplay` failed");
        return 1;
    }

    Window win = XCreateSimpleWindow(dpy, DefaultRootWindow(dpy), 100, 100, 300,
                                    200, 1, 0, 0xffffff);

    XStoreName(dpy, win, "Cursor test");
    XSelectInput(dpy, win, ExposureMask);
    XMapWindow(dpy, win);

    XcursorImages *img = XcursorFilenameLoadImages("left_ptr", 64);

    if (!img) {
        puts("`XcursorFilenameLoadImage` failed");
        return 1;
    }

    Cursor cur = XcursorImagesLoadCursor(dpy, img);
    XDefineCursor(dpy, win, cur);
    XFlush(dpy);

    getchar();

    XFreeCursor(dpy, cur);
    XcursorImagesDestroy(img);
    XCloseDisplay(dpy);
}
