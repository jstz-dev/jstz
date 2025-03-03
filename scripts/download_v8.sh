#!/bin/sh
ARCHIVE_URL="https://raw.githubusercontent.com/jstz-dev/rusty_v8/130.0.7/librusty_v8.tar.gz"
ARCHIVE_NAME="librusty_v8"
DIR=$(dirname $(realpath $0))
PARENT_DIR=$(dirname "$DIR")
echo $PARENT_DIR
if [ ! -f "$PARENT_DIR/librusty_v8.a" ]; then
    echo "Downloading v8 from $ARCHIVE_URL"
    wget -q -O "$DIR/$ARCHIVE_NAME.tar.gz" "$ARCHIVE_URL"
    tar -xzf "$DIR/$ARCHIVE_NAME.tar.gz" -C $DIR
    mv "$DIR/$ARCHIVE_NAME/librusty_v8.a" "$DIR/../librusty_v8.a"
    mv "$DIR/$ARCHIVE_NAME/src_binding.rs" "$DIR/../librusty_v8_src_binding.rs"
    rm -r $DIR/$ARCHIVE_NAME
    rm -r $DIR/$ARCHIVE_NAME.tar.gz
fi
