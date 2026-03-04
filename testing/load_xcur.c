#define _DEFAULT_SOURCE

#include <X11/X.h>
#include <X11/Xcursor/Xcursor.h>
#include <X11/Xlib.h>
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

// NOTE: this is C23

int main(int argc, char** argv) {
    char* cursor_name = "left_ptr";
    int32_t size = 32;

    const char* usage_yap = "usage: ./load_xcur [cursor_name=left_ptr] [size=32]\n";

    if (argc > 3) {
        fprintf(stderr, "too many arguments\n");
        fprintf(stderr, "%s", usage_yap);
        return 1;
    }

    if (argc >= 2) {
        cursor_name = argv[1];
    }

    if (argc == 3) {
        size = atoi(argv[2]);

        if (size <= 0) {
            fprintf(stderr, "size must be greater than zero (or invalid size arg)\n");
            fprintf(stderr, "%s", usage_yap);
            return 1;
        }
    }

    Display* display = XOpenDisplay(nullptr);

    if (!display) {
        fprintf(stderr, "XOpenDisplay() failed\n");
        return 1;
    }

    const int32_t WINDOW_X = 100;
    const int32_t WINDOW_Y = 100;
    const uint32_t WIDTH = 600;
    const uint32_t HEIGHT = 400;
    const uint32_t BORDER_WIDTH = 1;
    const uint64_t BORDER_COLOR = 0;              // black
    const uint64_t BACKGROUND_COLOR = 0xFF393a3c; // gray

    // create window to display cursor in
    const Window window = XCreateSimpleWindow(
        display, DefaultRootWindow(display),
        WINDOW_X, WINDOW_Y,
        WIDTH, HEIGHT,
        BORDER_WIDTH, BORDER_COLOR,
        BACKGROUND_COLOR);

    // set window name
    XStoreName(display, window, cursor_name);

    // make window visible
    XMapWindow(display, window);

    // if the cursor has multiple sizes, it displays the
    // size closest to `size`. note that this goes off of
    // the *nominal size* so technically ... blahblahblah
    XcursorSetDefaultSize(display, size);

    XcursorImages* images = nullptr;
    XcursorComments* comments = nullptr;

    if (!XcursorFilenameLoad(cursor_name, &comments, &images)) {
        fprintf(stderr, "XcursorFilenameLoad() failed\n");
        return 1;
    }

    // i don't think the returned pointers can be
    // null but i don't wanna risk ub so why not

    if (!images) {
        fprintf(stderr, "XcursorFilenameLoad() returned NULL for **images\n");
        return 1;
    }

    if (!comments) {
        fprintf(stderr, "XcursorFilenameLoad() returned NULL for **comments\n");
        return 1;
    }

    const size_t num_comments = (size_t) comments->ncomment;
    const size_t num_images = (size_t) images->nimage;

    for (size_t i = 0; i < num_comments; ++i) {
        const XcursorComment* cmt = comments->comments[i];
        assert(cmt->version == 1);

        fprintf(
            stderr, "[comment #%zu]: type=%u, comment=%s\n",
            i, cmt->comment_type, cmt->comment);
    }

    XcursorCommentsDestroy(comments);

    for (size_t i = 0; i < num_images; ++i) {
        const XcursorImage* img = images->images[i];
        assert(img->version == 1);

        fprintf(
            stderr, "[frame #%zu]: height=%u, width=%u, size=%u, xhot=%u, yhot=%u, delay=%u\n",
            i, img->height, img->width, img->size, img->xhot, img->yhot, img->delay);
    }

    XcursorImagesDestroy(images);

    // if this fails, the cursor just doesn't render. how sad
    Cursor cursor = XcursorFilenameLoadCursor(display, cursor_name);
    XDefineCursor(display, window, cursor);
    XFlush(display);

    printf("Press enter to exit, or just close the window ... ");
    getchar();

    return 0;
}
