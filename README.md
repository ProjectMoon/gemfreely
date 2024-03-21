# Gemfreely

Sync Gemini protocol Gemlogs to WriteFreely instances.

## Prerequisites, and who is this for?

To use `gemfreely`, You will need:

* A gemini capsule. See [geminiprotocol.net][1].
* A WriteFreely instance. See [writefreely.org][2].

`gemfreely` is for people who want to link their Gemini capsule's
gemlog into the Fediverse in a simple way. Gemini as a protocol is not
so easily accessible to most who use the internet, and with the
Fediverse gaining traction daily, this provides a way to increase the
visibility of your Gemini capsule's blog posts.

[What is Gemini?][1] Gemini is a lightweight alternative to the
regular Web, but not designed to replace it.

[What is WriteFreely?][2] WriteFreely is blogging software, similar to
WordPress, that integrates with the so-called "Fediverse"
(interconnected social media sites using the [ActivityPub][3]
standard).

## Usage

```
gemfreely help
```

Gemfreely is designed to be straightforward to use, and is mostly
intended for use in an automated fashion. There are two steps to
successfully synchronizing your gemlog to WriteFreely:

1. `gemfreely login`
2. `gemfreely sync`

### WriteFreely Login

First, you must possess an access token to your WriteFreely instance.
If you do not have one, `gemfreely` can create one for you:

```
gemfreely login --wf-url="https://witefreely.example.com/" -u yourusername -p "yourpassword"
```

If your login is successful, this will print out the WriteFreely
access token. Save this for use, as it is not stored anywhere.

### Sync Gemlog to WriteFreely

To synchronize your gemlog to WriteFreely, use the `sync` command. You will need:

* Your WriteFreely access token. **Note:** Username and password are
  *not* supported for sync.
* The full URL to your gemlog's feed (either Atom or Gemfeed, but Atom
  is preferable for publish date accuracy).
* The URL of your writefreely blog.

```
gemfreely \
  -t "YourWFAccessToken" \
  --wf-alias="yourusername" \
  sync \
  --gemlog-url="gemini://example.com/gemlog/atom.xml" \
  --wf-url="https://writefreely.example.com"
```

The `--wf-alias` argument, in WriteFreely terms, is the `alias` of
your WriteFreely `collection`. In more common terms, this is the
identifier of your blog, and it's usually the same as your WriteFreely
username.

#### Additional Options

The `sync` command has two additional options relating to sanitization
of the converted gemlog posts:

```
--strip-before-marker <STRIP_BEFORE_MARKER>
  Remove all text BEFORE this marker in the Gemlog post

--strip-after-marker <STRIP_AFTER_MARKER>
  Remove all text AFTER this marker in the Gemlog post
```

These markers can be any valid UTF-8 string. The primary purpose of
these options is to remove text that is present in the gemlog post,
but doesn't need to be in the converted blog post in Markdown. On my
gemlog, it is used to remove the title, footer, and header, which contain
links to go back to other parts of the Gemini capsule. These don't
need to be present in the WriteFreely post.

### Logout

It is possible to invalidate the WriteFreely access token by using `gemfreely logout`:

```
gemfreely --wf-alias="youralias"
  \ -t "YourAccessToken" logout \
  --wf-url="https://writefreely.example.com"
```

This will revoke the access token on WriteFreely, and a new one will
be required to use the `sync` command.

[1]: https://geminiprotocol.net/
[2]: https://writefreely.org/
[3]: https://en.wikipedia.org/wiki/ActivityPub
