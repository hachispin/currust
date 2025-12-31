#define _DEFAULT_SOURCE  // Add this at the very top
#include <X11/Xcursor/Xcursor.h>
#include <X11/Xlib.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main() {
    Display *dpy = XOpenDisplay(NULL);
    if (!dpy) {
        puts("`XOpenDisplay` failed");
        return 1;
    }

    Window win = XCreateSimpleWindow(dpy, DefaultRootWindow(dpy), 100, 100, 300, 200, 1, 0, 0xffffff);
    XStoreName(dpy, win, "Animated Cursor Test");
    XSelectInput(dpy, win, ExposureMask | StructureNotifyMask);
    XMapWindow(dpy, win);

    XcursorImages *img = XcursorFilenameLoadImages("left_ptr", 128);
    if (!img) {
        puts("`XcursorFilenameLoadImages` failed");
        return 1;
    }

    Cursor *frames = malloc(sizeof(Cursor) * (size_t)img->nimage);  // Fixed sign conversion
    for (int i = 0; i < img->nimage; i++)
        frames[i] = XcursorImageLoadCursor(dpy, img->images[i]);

    int frame = 0;
    while (1) {
        XDefineCursor(dpy, win, frames[frame]);
        XFlush(dpy);

        unsigned int delay = img->images[frame]->delay;
        if (delay == 0) delay = 100;
        usleep(delay * 1000);

        frame = (frame + 1) % img->nimage;
    }

    for (int i = 0; i < img->nimage; i++)
        XFreeCursor(dpy, frames[i]);
    free(frames);
    XcursorImagesDestroy(img);
    XCloseDisplay(dpy);
}
