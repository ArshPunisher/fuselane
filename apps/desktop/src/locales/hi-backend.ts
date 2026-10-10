// Hindi for text the Rust core sends: errors and their hints, a download's failure and
// notes, feed and watch-folder results, Nearby and Send problems. The core writes English
// (the CLI and logs use it too); the app translates it here, where it's shown (tb() in
// lib/i18n.ts). `{names}` mark the parts that change: a template is matched whole, and a
// part named why, e, last, reason, problem, next, what, todo or m is a sentence from the
// core too, translated in turn. Text not here stays in English.
// How to add one, and what's left in English: docs/07-design/I18N.md.
// scripts/i18n-check.ts checks every entry (Hindi present, same {names}, no dashes).
export const hiBackend: Readonly<Record<string, string>> = {
  // ==== Adding a download ====
  'Paste a link to download.': 'डाउनलोड करने के लिए कोई लिंक पेस्ट करें।',
  'Links start with http:// or https://.': 'लिंक http:// या https:// से शुरू होते हैं।',
  'That link is too long (over 8 KB).': 'यह लिंक बहुत लंबा है (8 KB से ज़्यादा)।',
  'That link is too long.': 'यह लिंक बहुत लंबा है।',
  'Copy the link again from its page.': 'लिंक को उसके पेज से फिर से कॉपी करें।',
  '"{link}" isn\'t a valid link.': '"{link}" सही लिंक नहीं है।',
  "That isn't a valid link.": 'यह सही लिंक नहीं है।',
  "{scheme}: links aren't supported. Use an http:// or https:// link.":
    '{scheme}: लिंक नहीं चलते। http:// या https:// लिंक इस्तेमाल करें।',
  'The link has no host name.': 'लिंक में होस्ट का नाम नहीं है।',
  'No usable networks. Join a Wi-Fi network, plug in Ethernet, or tether a phone over USB.':
    'इस्तेमाल लायक कोई नेटवर्क नहीं है। किसी Wi-Fi नेटवर्क से जुड़ें, Ethernet लगाएं, या USB से फ़ोन टेदर करें।',
  'couldn\'t find the server "{host}". Check the link and your connection.':
    'सर्वर "{host}" नहीं मिला। लिंक और अपना कनेक्शन जांचें।',
  "A mirror link isn't usable: {why}": 'एक मिरर लिंक काम का नहीं है: {why}',
  'Mirrors are http:// or https:// links to the same file.':
    'मिरर उसी फ़ाइल के http:// या https:// लिंक होते हैं।',
  '"{link}" isn\'t a web link.': '"{link}" वेब लिंक नहीं है।',
  "That's more than 8 mirrors.": 'ये 8 से ज़्यादा मिरर हैं।',
  'Keep the fastest few; more rarely helps.':
    'सबसे तेज़ कुछ ही रखें, ज़्यादा रखने से शायद ही फ़ायदा होता है।',
  'That file name is too long (over 255 bytes).':
    'फ़ाइल का नाम बहुत लंबा है (255 बाइट से ज़्यादा)।',
  'Pick a shorter name.': 'छोटा नाम चुनें।',
  'Paste the SHA-256 from the download page: 64 letters and digits.':
    'डाउनलोड पेज से SHA-256 पेस्ट करें: 64 अक्षर और अंक।',
  'a SHA-256 is 64 hexadecimal characters': 'SHA-256 में 64 हेक्साडेसिमल अक्षर होते हैं',
  'A SHA-256 is 64 hex digits (0-9, a-f).': 'SHA-256 में 64 हेक्स अंक होते हैं (0-9, a-f)।',
  "The browser's request can't be used: {m}.": 'ब्राउज़र का अनुरोध इस्तेमाल नहीं हो सकता: {m}।',
  "the login in the link isn't valid": 'लिंक में दिया लॉगिन सही नहीं है',
  "the {header} header can't be passed on": '{header} हेडर आगे नहीं भेजा जा सकता',
  "the {header} header has characters that aren't allowed":
    '{header} हेडर में ऐसे अक्षर हैं जिनकी अनुमति नहीं है',
  'the headers are too large (over 16 KB)': 'हेडर बहुत बड़े हैं (16 KB से ज़्यादा)',
  'You already added this link ({name}).': 'आप यह लिंक पहले ही जोड़ चुके हैं ({name})।',
  'Download it again anyway, or open the one in your list.':
    'फिर भी दोबारा डाउनलोड करें, या अपनी सूची वाला खोलें।',
  'The folder "{path}" doesn\'t exist.': 'फ़ोल्डर "{path}" मौजूद नहीं है।',
  'Pick another folder to save into.': 'सेव करने के लिए कोई दूसरा फ़ोल्डर चुनें।',
  "That's more than {n} links at once.": 'एक बार में {n} से ज़्यादा लिंक हैं।',
  'Add them in smaller groups.': 'इन्हें छोटे-छोटे हिस्सों में जोड़ें।',
  'There are no http:// or https:// links in that text.':
    'इस टेक्स्ट में कोई http:// या https:// लिंक नहीं है।',
  'Paste one link per line, or a pattern like https://example.com/part[01-10].zip.':
    'हर लाइन में एक लिंक पेस्ट करें, या https://example.com/part[01-10].zip जैसा पैटर्न।',
  'That pattern makes more than {n} links. Use a smaller range.':
    'यह पैटर्न {n} से ज़्यादा लिंक बनाता है। छोटी रेंज इस्तेमाल करें।',
  "That's too much text to read links from (over 1 MB).":
    'लिंक पढ़ने के लिए यह टेक्स्ट बहुत ज़्यादा है (1 MB से ज़्यादा)।',
  'Paste the links in smaller groups.': 'लिंक छोटे-छोटे हिस्सों में पेस्ट करें।',
  'That file is {n} KB; a list of links is at most 1 MB.':
    'यह फ़ाइल {n} KB की है, लिंक की सूची ज़्यादा से ज़्यादा 1 MB की हो सकती है।',
  'Pick the text file with the links, or split it into smaller files.':
    'लिंक वाली टेक्स्ट फ़ाइल चुनें, या इसे छोटी फ़ाइलों में बांटें।',
  "The server sent the download to something that isn't a link.":
    'सर्वर ने डाउनलोड को ऐसी जगह भेजा जो लिंक नहीं है।',
  "The server sent the download to a {scheme}: link, which Fuselane doesn't open.":
    'सर्वर ने डाउनलोड को {scheme}: लिंक पर भेजा, जिसे Fuselane नहीं खोलता।',
  'The server redirected the download more than {n} times, so it may be broken. Check the link.':
    'सर्वर ने डाउनलोड को {n} से ज़्यादा बार दूसरी जगह भेजा, इसलिए यह शायद टूटा हुआ है। लिंक जांचें।',
  'No network can reach that site.': 'कोई भी नेटवर्क उस साइट तक नहीं पहुंच पा रहा।',
  "Couldn't read that address: it didn't answer, isn't text, or is over {n} MB.":
    'वह पता पढ़ा नहीं जा सका: उसने जवाब नहीं दिया, वह टेक्स्ट नहीं है, या {n} MB से बड़ा है।',
  "That page doesn't link to any files.": 'इस पेज पर किसी फ़ाइल का लिंक नहीं है।',
  'Paste the link of the file itself, or a page that lists downloads.':
    'सीधे फ़ाइल का लिंक पेस्ट करें, या ऐसा पेज जिस पर डाउनलोड की सूची हो।',
  'This download is already running in Fuselane or in another terminal. Pause it there first.':
    'यह डाउनलोड Fuselane में या किसी दूसरे टर्मिनल में पहले से चल रहा है। पहले उसे वहां रोकें।',
  "download {id} is {status} and can't be resumed. Start it again.":
    'डाउनलोड {id} की स्थिति {status} है, इसलिए यह आगे नहीं बढ़ सकता। इसे दोबारा शुरू करें।',

  // ==== Why a download failed (core::describe) ====
  "Couldn't reach the server on any selected network ({why}). Check your connection and the link.":
    'चुने गए किसी भी नेटवर्क से सर्वर तक नहीं पहुंच पाए ({why})। अपना कनेक्शन और लिंक जांचें।',
  "The server says this file doesn't exist (404). Check the link.":
    'सर्वर के मुताबिक यह फ़ाइल मौजूद नहीं है (404)। लिंक जांचें।',
  'The server refused access to this file. The link may need you to be signed in.':
    'सर्वर ने इस फ़ाइल तक पहुंचने नहीं दिया। हो सकता है लिंक के लिए साइन इन करना ज़रूरी हो।',
  'The server is limiting downloads right now (429 Too Many Requests). Wait a few minutes, then resume.':
    'सर्वर अभी डाउनलोड सीमित कर रहा है (429 Too Many Requests)। कुछ मिनट रुकें, फिर दोबारा शुरू करें।',
  'The server is busy (status {status}). Wait a minute, then resume.':
    'सर्वर व्यस्त है (स्टेटस {status})। एक मिनट रुकें, फिर दोबारा शुरू करें।',
  "The server answered with status {status}, so the download couldn't start.":
    'सर्वर ने स्टेटस {status} के साथ जवाब दिया, इसलिए डाउनलोड शुरू नहीं हो सका।',
  'The server kept sending the download elsewhere (status {status}). Check the link.':
    'सर्वर डाउनलोड को बार-बार दूसरी जगह भेजता रहा (स्टेटस {status})। लिंक जांचें।',
  'This link stopped working (the server said {status}). Get a fresh link to the same file and try again.':
    'इस लिंक ने काम करना बंद कर दिया (सर्वर ने {status} कहा)। उसी फ़ाइल का नया लिंक लें और फिर से कोशिश करें।',
  'The file on the server changed during the download, so it was stopped to avoid a mixed file. Start it again.':
    'डाउनलोड के दौरान सर्वर पर फ़ाइल बदल गई, इसलिए मिली-जुली फ़ाइल से बचने के लिए इसे रोक दिया गया। इसे दोबारा शुरू करें।',
  'Not enough free space on that disk: this download needs {needed} more and only {free} is free. Free up space or choose another folder.':
    'उस डिस्क पर जगह कम है: इस डाउनलोड को {needed} और चाहिए, जबकि सिर्फ़ {free} खाली है। जगह खाली करें या कोई दूसरा फ़ोल्डर चुनें।',
  'Saving failed ({detail}). Check free space and that the folder is writable.':
    'सेव नहीं हो सका ({detail})। खाली जगह जांचें, और यह भी कि फ़ोल्डर में लिखा जा सकता है।',
  'Every network failed. Last problem: {last}': 'हर नेटवर्क विफल रहा। आखिरी समस्या: {last}',
  "The downloaded file doesn't match the SHA-256 you gave, so it wasn't saved under its name. The partial file is kept for inspection.":
    'डाउनलोड की गई फ़ाइल आपके दिए SHA-256 से मेल नहीं खाती, इसलिए उसे उसके नाम से सेव नहीं किया गया। अधूरी फ़ाइल जांच के लिए रखी गई है।',
  'Paused. Progress is saved.': 'रुका हुआ। अब तक का डाउनलोड सेव है।',
  "This download can't be resumed ({why}). Start it again.":
    'यह डाउनलोड आगे नहीं बढ़ सकता ({why})। इसे दोबारा शुरू करें।',
  "Couldn't save the file: {e}": 'फ़ाइल सेव नहीं हो सकी: {e}',
  'no networks selected': 'कोई नेटवर्क नहीं चुना गया',
  'no network could reach the server': 'कोई भी नेटवर्क सर्वर तक नहीं पहुंच सका',
  'no network could reach the server: {why}': 'कोई भी नेटवर्क सर्वर तक नहीं पहुंच सका: {why}',
  'the server no longer supports resuming': 'सर्वर अब बीच से आगे बढ़ाने की सुविधा नहीं देता',
  'the partial file is missing': 'अधूरी फ़ाइल नहीं मिली',
  "the folder {path} doesn't exist": 'फ़ोल्डर {path} मौजूद नहीं है',
  'refusing to publish: {written} of {expected} bytes are on disk':
    'फ़ाइल पूरी नहीं है: डिस्क पर {expected} में से सिर्फ़ {written} बाइट हैं',
  "Downloaded, but it couldn't be unpacked: {why}.":
    'डाउनलोड हो गया, पर इसे खोला (अनपैक) नहीं जा सका: {why}।',
  'it is password-protected; unpack it yourself': 'इस पर पासवर्ड लगा है, इसे खुद खोलें',
  'it holds an unsafe path ({path})': 'इसमें एक असुरक्षित पाथ है ({path})',
  'Fuselane keeps trying.': 'Fuselane कोशिश करता रहेगा।',

  // Mirrors: why one wasn't used (a download's detail)
  "Mirrors weren't used: this download carries a sign-in for its own site.":
    'मिरर इस्तेमाल नहीं हुए: इस डाउनलोड में उसकी अपनी साइट का साइन-इन है।',
  "Mirrors weren't used: the main server doesn't let a file be split.":
    'मिरर इस्तेमाल नहीं हुए: मुख्य सर्वर फ़ाइल को हिस्सों में बांटने नहीं देता।',
  "{host} wasn't used: {why}.": '{host} इस्तेमाल नहीं हुआ: {why}।',
  'it has a different file size': 'उसकी फ़ाइल का साइज़ अलग है',
  "it doesn't let the file be split": 'वह फ़ाइल को हिस्सों में बांटने नहीं देता',
  "its bytes differ from the main server's": 'उसके बाइट मुख्य सर्वर से अलग हैं',

  // ==== A network's proxy (core::proxy, and checking one in Networks) ====
  "{network}'s proxy, {host}, couldn't be found.": '{network} का प्रॉक्सी, {host}, नहीं मिला।',
  'Check its name in Networks, under Proxy.': 'नेटवर्क में, प्रॉक्सी के नीचे उसका नाम जांचें।',
  "{network} couldn't connect to its proxy at {at} ({reason}).":
    '{network} अपने प्रॉक्सी {at} से कनेक्ट नहीं हो सका ({reason})।',
  'Check the address and port, and that the proxy is running.':
    'पता और पोर्ट जांचें, और देखें कि प्रॉक्सी चल रहा है।',
  "{network}'s proxy at {at} wants a username and password.":
    '{at} पर {network} का प्रॉक्सी यूज़रनेम और पासवर्ड मांग रहा है।',
  'Add them in Networks, under Proxy.': 'इन्हें नेटवर्क में, प्रॉक्सी के नीचे जोड़ें।',
  "{network}'s proxy at {at} turned down the username and password.":
    '{at} पर {network} के प्रॉक्सी ने यूज़रनेम और पासवर्ड नहीं माना।',
  'Check them in Networks, under Proxy.': 'इन्हें नेटवर्क में, प्रॉक्सी के नीचे जांचें।',
  "{network}'s proxy refused to connect to {target}.":
    '{network} के प्रॉक्सी ने {target} से कनेक्ट करने से मना कर दिया।',
  'Its rules may allow only some sites or ports (many allow only https links). Ask whoever runs it, or remove the proxy.':
    'हो सकता है उसके नियम कुछ ही साइटों या पोर्ट की अनुमति देते हों (कई सिर्फ़ https लिंक की अनुमति देते हैं)। उसे चलाने वाले से पूछें, या प्रॉक्सी हटा दें।',
  "{network}'s proxy couldn't reach {target}.":
    '{network} का प्रॉक्सी {target} तक नहीं पहुंच पाया।',
  "The site may be down, or the proxy can't get out right now.":
    'हो सकता है साइट बंद हो, या प्रॉक्सी अभी बाहर नहीं जा पा रहा।',
  "{at} doesn't answer like a {kind} proxy.": '{at} किसी {kind} प्रॉक्सी की तरह जवाब नहीं देता।',
  'connection refused': 'कनेक्शन से मना कर दिया गया',
  'Check the type (HTTP or SOCKS5) and the port in Networks, under Proxy.':
    'नेटवर्क में, प्रॉक्सी के नीचे प्रकार (HTTP या SOCKS5) और पोर्ट जांचें।',
  "{network}'s proxy wants a sign-in method Fuselane doesn't support.":
    '{network} का प्रॉक्सी साइन इन का ऐसा तरीका मांग रहा है जो Fuselane में नहीं है।',
  'Fuselane signs in with a username and password. Use another proxy, or remove this one.':
    'Fuselane यूज़रनेम और पासवर्ड से साइन इन करता है। कोई दूसरा प्रॉक्सी इस्तेमाल करें, या इसे हटा दें।',
  "{network}'s proxy at {at} didn't answer in time.":
    '{at} पर {network} के प्रॉक्सी ने समय पर जवाब नहीं दिया।',
  'Check the address, port and type, and that the proxy is running.':
    'पता, पोर्ट और प्रकार जांचें, और देखें कि प्रॉक्सी चल रहा है।',
  "Enter the proxy's name or address.": 'प्रॉक्सी का नाम या पता लिखें।',
  'For example proxy.example.com or 192.168.1.10.': 'जैसे proxy.example.com या 192.168.1.10।',
  "Enter just the proxy's name or address.": 'सिर्फ़ प्रॉक्सी का नाम या पता लिखें।',
  'Leave out http:// or socks5://; choose the type above instead.':
    'http:// या socks5:// न लिखें, इसकी जगह ऊपर प्रकार चुनें।',
  "That address can't be a proxy.": 'यह पता प्रॉक्सी नहीं हो सकता।',
  'Use the address of the computer that runs the proxy.':
    'उस कंप्यूटर का पता लिखें जिस पर प्रॉक्सी चल रहा है।',
  'Put the port in the Port box, not after the name.':
    'पोर्ट को नाम के बाद नहीं, पोर्ट वाले बॉक्स में लिखें।',
  'For example proxy.example.com, and 8080 as the port.':
    'जैसे proxy.example.com, और पोर्ट में 8080।',
  "That isn't a valid proxy name.": 'यह प्रॉक्सी का सही नाम नहीं है।',
  'Use letters, numbers, dots and dashes, like proxy.example.com.':
    'अक्षर, अंक, बिंदु और डैश इस्तेमाल करें, जैसे proxy.example.com।',
  "That port isn't valid.": 'यह पोर्ट सही नहीं है।',
  'Ports go from 1 to 65535; proxies often use 8080, 3128 or 1080.':
    'पोर्ट 1 से 65535 तक होते हैं, प्रॉक्सी अक्सर 8080, 3128 या 1080 इस्तेमाल करते हैं।',
  "That username isn't valid.": 'यह यूज़रनेम सही नहीं है।',
  'Use up to 255 characters, without control characters.':
    '255 अक्षरों तक लिखें, बिना कंट्रोल कैरेक्टर के।',
  "An HTTP proxy's username can't contain a colon.":
    'HTTP प्रॉक्सी के यूज़रनेम में कोलन (:) नहीं हो सकता।',
  'Check the username your proxy gave you.': 'अपने प्रॉक्सी से मिला यूज़रनेम जांचें।',
  'Add the username that goes with the password.': 'पासवर्ड के साथ वाला यूज़रनेम भी जोड़ें।',
  "Or clear the password if the proxy doesn't need one.":
    'या, अगर प्रॉक्सी को पासवर्ड नहीं चाहिए, तो पासवर्ड हटा दें।',
  "That password isn't valid.": 'यह पासवर्ड सही नहीं है।',
  "There's no proxy set for that network.": 'उस नेटवर्क के लिए कोई प्रॉक्सी सेट नहीं है।',
  'Set one first, then check it.': 'पहले एक सेट करें, फिर उसे जांचें।',
  "{network} isn't connected right now, so its proxy can't be checked.":
    '{network} अभी कनेक्ट नहीं है, इसलिए उसका प्रॉक्सी जांचा नहीं जा सकता।',
  'Connect it, then check again.': 'उसे कनेक्ट करें, फिर दोबारा जांचें।',
  'Works: {network} reaches the internet through this proxy.':
    'काम कर रहा है: {network} इस प्रॉक्सी से इंटरनेट तक पहुंच रहा है।',

  // ==== Networks, speed limits and allowances ====
  'Use up to 40 characters.': '40 अक्षरों तक लिखें।',
  "That isn't a network on this computer.": 'यह इस कंप्यूटर का नेटवर्क नहीं है।',
  'That name is too long.': 'यह नाम बहुत लंबा है।',
  "Names can't contain control characters.": 'नाम में कंट्रोल कैरेक्टर नहीं हो सकते।',
  'Pick a start and a stop time that are different.': 'शुरू और बंद होने का अलग-अलग समय चुनें।',
  "That colour isn't one of Fuselane's network colours.":
    'यह रंग Fuselane के नेटवर्क रंगों में से नहीं है।',
  "That isn't a way to use a network.": 'यह नेटवर्क इस्तेमाल करने का कोई तरीका नहीं है।',
  'Use a speed in KB/s or MB/s, or leave it empty for no limit.':
    'स्पीड KB/s या MB/s में लिखें, या कोई लिमिट न रखने के लिए खाली छोड़ें।',
  'That overall limit is too high to be a real speed.':
    'कुल लिमिट इतनी ज़्यादा है कि यह असली स्पीड नहीं हो सकती।',
  'That slow-mode speed is too high to be a real speed.':
    'स्लो मोड की स्पीड इतनी ज़्यादा है कि यह असली स्पीड नहीं हो सकती।',
  'Too many network limits.': 'नेटवर्क लिमिट बहुत ज़्यादा हैं।',
  'A network limit has no valid network name.': 'एक नेटवर्क लिमिट में सही नेटवर्क का नाम नहीं है।',
  'That network limit is too high to be a real speed.':
    'यह नेटवर्क लिमिट इतनी ज़्यादा है कि यह असली स्पीड नहीं हो सकती।',
  'A network is listed twice.': 'एक नेटवर्क दो बार लिखा है।',
  'That limit is too high to be a real speed.':
    'यह लिमिट इतनी ज़्यादा है कि यह असली स्पीड नहीं हो सकती।',
  'Use an amount like 5 GB and a reset day from 1 to 28.':
    '5 GB जैसी मात्रा और 1 से 28 के बीच रीसेट का दिन लिखें।',
  'The reset day must be from 1 to 28.': 'रीसेट का दिन 1 से 28 के बीच होना चाहिए।',
  'That allowance is too big to be real.': 'यह डेटा लिमिट इतनी बड़ी है कि असली नहीं हो सकती।',
  'Too many allowances.': 'डेटा लिमिट बहुत ज़्यादा हैं।',
  'Too many renamed networks.': 'नाम बदले गए नेटवर्क बहुत ज़्यादा हैं।',
  "Couldn't list your networks: {e}": 'आपके नेटवर्क की सूची नहीं मिल सकी: {e}',

  // ==== Downloads: actions, queue and schedule ====
  "There's no download {id}.": 'डाउनलोड {id} नहीं मिला।',
  'It may have been removed already.': 'शायद इसे पहले ही हटा दिया गया है।',
  "Fuselane couldn't read or save its list of downloads: {e}":
    'Fuselane अपनी डाउनलोड सूची पढ़ या सेव नहीं कर सका: {e}',
  "This download isn't running, so there's nothing to pause.":
    'यह डाउनलोड चल नहीं रहा, इसलिए रोकने को कुछ नहीं है।',
  'Add the link again to download it from the start.':
    'शुरू से डाउनलोड करने के लिए लिंक फिर से जोड़ें।',
  'Pause the download before changing its link.': 'लिंक बदलने से पहले डाउनलोड रोकें।',
  "This download can't continue from a new link.": 'यह डाउनलोड नए लिंक से आगे नहीं बढ़ सकता।',
  'Start it again instead.': 'इसकी जगह इसे दोबारा शुरू करें।',
  'Pause the download before starting it over.': 'दोबारा शुरू करने से पहले डाउनलोड रोकें।',
  'This download already finished.': 'यह डाउनलोड पहले ही पूरा हो चुका है।',
  "Couldn't move the file to the Trash: {e}": 'फ़ाइल ट्रैश में नहीं जा सकी: {e}',
  'Delete it from its folder instead, then remove it from the list.':
    'इसकी जगह उसे उसके फ़ोल्डर से मिटाएं, फिर सूची से हटाएं।',
  "This download hasn't finished yet, so there's no file to show.":
    'यह डाउनलोड अभी पूरा नहीं हुआ, इसलिए दिखाने को कोई फ़ाइल नहीं है।',
  "This download hasn't finished yet, so there's no file.":
    'यह डाउनलोड अभी पूरा नहीं हुआ, इसलिए कोई फ़ाइल नहीं है।',
  'The file is no longer at {path}.': 'फ़ाइल अब {path} पर नहीं है।',
  'It may have been moved, renamed or deleted.':
    'शायद उसे कहीं और ले जाया गया, उसका नाम बदला गया या उसे मिटा दिया गया।',
  "That file isn't here anymore.": 'यह फ़ाइल अब यहां नहीं है।',
  "Couldn't show the file: {e}": 'फ़ाइल दिखाई नहीं जा सकी: {e}',
  "Couldn't open the file: {e}": 'फ़ाइल खुल नहीं सकी: {e}',
  'Couldn\'t read "{name}": {e}': '"{name}" पढ़ी नहीं जा सकी: {e}',
  'Couldn\'t save "{name}": {e}': '"{name}" सेव नहीं हो सकी: {e}',
  'Pick a folder you can write to.': 'ऐसा फ़ोल्डर चुनें जिसमें आप लिख सकें।',
  'Pick between 1 and {n} downloads at once.': 'एक साथ 1 से {n} डाउनलोड चुनें।',
  'Too many downloads to reorder.': 'क्रम बदलने के लिए डाउनलोड बहुत ज़्यादा हैं।',
  'Waiting for the schedule. {next}': 'शेड्यूल का इंतज़ार है। {next}',
  'Starts at {when}.': '{when} पर शुरू होगा।',
  'Starts at {when} tomorrow.': 'कल {when} पर शुरू होगा।',
  'Starts at {when} in {n} days.': '{n} दिन बाद {when} पर शुरू होगा।',
  'Pauses at {when}.': '{when} पर रुकेगा।',
  'Pauses at {when} tomorrow.': 'कल {when} पर रुकेगा।',
  'Pauses at {when} in {n} days.': '{n} दिन बाद {when} पर रुकेगा।',
  'Paused: the battery is low. It carries on when you plug in.':
    'रुका हुआ: बैटरी कम है। चार्जर लगाने पर यह फिर चलेगा।',
  'Paused: every network reached its data allowance. It continues after the allowance resets, or raise it in Networks.':
    'रुका हुआ: हर नेटवर्क अपनी डेटा लिमिट तक पहुंच गया। लिमिट रीसेट होने पर यह फिर चलेगा, या नेटवर्क में लिमिट बढ़ाएं।',
  'Times must be between 00:00 and 23:59.': 'समय 00:00 से 23:59 के बीच होना चाहिए।',
  'The schedule starts and stops at the same time. Pick a stop time after the start.':
    'शेड्यूल एक ही समय पर शुरू और बंद होता है। शुरू होने के बाद का बंद होने का समय चुनें।',
  'Pick at least one day for the schedule.': 'शेड्यूल के लिए कम से कम एक दिन चुनें।',
  'Pick between 1 and 600 minutes.': '1 से 600 मिनट के बीच चुनें।',
  'Pick a time in the next year.': 'अगले एक साल के अंदर का समय चुनें।',
  "This download can't run, so it can't go first.":
    'यह डाउनलोड चल नहीं सकता, इसलिए पहले नहीं जा सकता।',
  'Download it again to start it from the beginning.':
    'शुरू से चलाने के लिए इसे फिर से डाउनलोड करें।',
  'Waiting: {name} goes first. This carries on after it.':
    'इंतज़ार में: पहले {name} चलेगा। उसके बाद यह चलेगा।',
  'Use up to 80 characters.': '80 अक्षरों तक लिखें।',
  'That group name is too long.': 'ग्रुप का नाम बहुत लंबा है।',
  "Group names can't contain control characters.":
    'ग्रुप के नाम में कंट्रोल कैरेक्टर नहीं हो सकते।',
  'Give the group a name.': 'ग्रुप को कोई नाम दें।',
  "This download hasn't started yet, so there's nothing to hand over.":
    'यह डाउनलोड अभी शुरू नहीं हुआ, इसलिए सौंपने को कुछ नहीं है।',
  'Send the link instead, or start it for a moment first.':
    'इसकी जगह लिंक भेजें, या पहले इसे थोड़ी देर चलाएं।',
  'Handed over from another computer. Resume to carry on from where it was.':
    'दूसरे कंप्यूटर से सौंपा गया। जहां रुका था वहीं से आगे बढ़ाने के लिए फिर शुरू करें।',
  'Pause the download first, then send it.': 'पहले डाउनलोड रोकें, फिर उसे भेजें।',

  // ==== Videos from pages, Metalink ====
  'That choice has expired. Look the page up again.': 'यह विकल्प पुराना हो गया। पेज फिर से खोजें।',
  'Saved as separate video and audio: install ffmpeg to join them next time.':
    'वीडियो और ऑडियो अलग-अलग सेव हुए: अगली बार उन्हें जोड़ने के लिए ffmpeg इंस्टॉल करें।',
  '{why}; the video and audio are kept as they are.':
    '{why}, वीडियो और ऑडियो जैसे थे वैसे ही रखे गए हैं।',
  "ffmpeg couldn't join them": 'ffmpeg उन्हें जोड़ नहीं सका',
  "ffmpeg couldn't make an MP3 (its MP3 encoder may be missing)":
    'ffmpeg MP3 नहीं बना सका (शायद उसका MP3 एनकोडर नहीं है)',
  "Videos from pages need the free yt-dlp tool, which isn't installed.":
    'पेज से वीडियो लेने के लिए मुफ़्त yt-dlp टूल चाहिए, जो इंस्टॉल नहीं है।',
  'Install it with Homebrew (brew install yt-dlp), then try again.':
    'इसे Homebrew से इंस्टॉल करें (brew install yt-dlp), फिर से कोशिश करें।',
  'Install it with winget (winget install yt-dlp), then try again.':
    'इसे winget से इंस्टॉल करें (winget install yt-dlp), फिर से कोशिश करें।',
  'Install it from your package manager (yt-dlp), then try again.':
    'इसे अपने पैकेज मैनेजर से इंस्टॉल करें (yt-dlp), फिर से कोशिश करें।',
  'No video found there: {why}.': 'वहां कोई वीडियो नहीं मिला: {why}।',
  'unsupported URL': 'यह URL नहीं चलता',
  "Paste the link of the video's own page.": 'वीडियो के अपने पेज का लिंक पेस्ट करें।',
  "That page's video can't be downloaded in parts (it streams in pieces only).":
    'इस पेज का वीडियो हिस्सों में डाउनलोड नहीं हो सकता (यह सिर्फ़ टुकड़ों में स्ट्रीम होता है)।',
  'Download the .meta4 or .metalink file again from its page.':
    '.meta4 या .metalink फ़ाइल उसके पेज से फिर से डाउनलोड करें।',
  'This Metalink has no files Fuselane can download (http or https).':
    'इस Metalink में ऐसी कोई फ़ाइल नहीं है जिसे Fuselane डाउनलोड कर सके (http या https)।',
  "This Metalink can't be read ({e}).": 'यह Metalink पढ़ा नहीं जा सकता ({e})।',
  "This isn't a Metalink file.": 'यह Metalink फ़ाइल नहीं है।',

  // ==== Network check ====
  'Starting…': 'शुरू हो रहा है…',
  'Stopping…': 'रुक रहा है…',
  'Testing {network}…': '{network} की जांच हो रही है…',
  'Testing every network together…': 'सभी नेटवर्क की एक साथ जांच हो रही है…',
  'Run a network check first.': 'पहले नेटवर्क जांच चलाएं।',
  'Sign-in page in the way': 'बीच में साइन-इन पेज है',
  'No internet': 'इंटरनेट नहीं है',
  'No answer through this network: it may be offline or behind a sign-in page.':
    'इस नेटवर्क से कोई जवाब नहीं आया: शायद यह ऑफ़लाइन है या साइन-इन पेज के पीछे है।',
  "The speed test didn't finish: {e}": 'स्पीड टेस्ट पूरा नहीं हुआ: {e}',
  'Skipped: this network has used its data allowance for the month.':
    'छोड़ा गया: इस नेटवर्क ने इस महीने का अपना डेटा इस्तेमाल कर लिया है।',
  "Couldn't look up the speed server.": 'स्पीड सर्वर नहीं मिला।',
  "Couldn't look up the speed server: {e}": 'स्पीड सर्वर नहीं मिला: {e}',
  'The speed server answered {status}.': 'स्पीड सर्वर ने {status} जवाब दिया।',
  'The server closed the connection.': 'सर्वर ने कनेक्शन बंद कर दिया।',
  "The server's answer didn't make sense.": 'सर्वर का जवाब समझ में नहीं आया।',
  'No address for the speed server.': 'स्पीड सर्वर का कोई पता नहीं मिला।',
  "The speed server didn't answer in time.": 'स्पीड सर्वर ने समय पर जवाब नहीं दिया।',
  'Too little got through to measure.': 'मापने के लिए बहुत कम डेटा आया।',

  // ==== Feeds ====
  'Only http and https links to .torrent files are fetched.':
    'सिर्फ़ .torrent फ़ाइलों के http और https लिंक लाए जाते हैं।',
  'That link opens a web page, not a .torrent file.':
    'यह लिंक .torrent फ़ाइल नहीं, एक वेब पेज खोलता है।',
  'Open it in Fuselane to pick the file yourself.':
    'फ़ाइल खुद चुनने के लिए इसे Fuselane में खोलें।',
  "Fuselane couldn't save the feeds: {e}": 'Fuselane फ़ीड सेव नहीं कर सका: {e}',
  'Check that your disk has free space, then try again.':
    'देखें कि डिस्क में खाली जगह है, फिर से कोशिश करें।',
  'Check every 15 minutes, hour, 6 hours or day.': 'हर 15 मिनट, घंटे, 6 घंटे या दिन में जांचें।',
  "Paste the feed's address.": 'फ़ीड का पता पेस्ट करें।',
  'Feed addresses start with http:// or https:// and often end in /feed, .rss or .xml.':
    'फ़ीड के पते http:// या https:// से शुरू होते हैं और अक्सर /feed, .rss या .xml पर खत्म होते हैं।',
  'You already follow this feed.': 'आप यह फ़ीड पहले से फ़ॉलो कर रहे हैं।',
  'Up to {n} feeds can be followed.': 'ज़्यादा से ज़्यादा {n} फ़ीड फ़ॉलो की जा सकती हैं।',
  'Remove one you no longer need.': 'जिसकी अब ज़रूरत नहीं, उसे हटाएं।',
  'Check the address: it should open as a feed (RSS or Atom), not a web page.':
    'पता जांचें: यह वेब पेज नहीं, फ़ीड (RSS या Atom) की तरह खुलना चाहिए।',
  "That feed isn't followed anymore.": 'यह फ़ीड अब फ़ॉलो नहीं की जा रही।',
  "It couldn't be started.": 'इसे शुरू नहीं किया जा सका।',
  'Already in the list.': 'पहले से सूची में है।',
  'Already in your torrents.': 'पहले से आपके टोरेंट में है।',
  'Fuselane is closing.': 'Fuselane बंद हो रहा है।',
  'That link is over {n} MB, too big for a .torrent file.':
    'यह लिंक {n} MB से बड़ा है, .torrent फ़ाइल के लिए बहुत बड़ा।',
  'Open it in Fuselane to see what it is.': 'यह क्या है, देखने के लिए इसे Fuselane में खोलें।',
  "Couldn't fetch the .torrent file: {why}": '.torrent फ़ाइल नहीं लाई जा सकी: {why}',
  'Fuselane tries again only if you open it yourself.':
    'Fuselane दोबारा तभी कोशिश करेगा जब आप इसे खुद खोलेंगे।',
  "This isn't a readable feed ({e}).": 'यह पढ़ने लायक फ़ीड नहीं है ({e})।',
  "This isn't a feed: it's not RSS or Atom.": 'यह फ़ीड नहीं है: यह RSS या Atom नहीं है।',

  // ==== Add files from a folder ====
  'Pick a folder to watch.': 'नज़र रखने के लिए कोई फ़ोल्डर चुनें।',
  'Choose an existing folder, such as one your media tool drops files into.':
    'कोई मौजूदा फ़ोल्डर चुनें, जैसे वह जिसमें आपका मीडिया टूल फ़ाइलें रखता है।',
  "That's your downloads folder, where finished .torrent and .txt files land.":
    'यह आपका डाउनलोड फ़ोल्डर है, जहां पूरी हुई .torrent और .txt फ़ाइलें आती हैं।',
  "Pick a separate folder, so downloads aren't taken as new work.":
    'अलग फ़ोल्डर चुनें, ताकि डाउनलोड को नया काम न समझा जाए।',
  "The folder can't be read ({e}).": 'फ़ोल्डर पढ़ा नहीं जा सकता ({e})।',
  'Not added: {why}.': 'नहीं जोड़ा गया: {why}।',
  'Torrent started: {name}.': 'टोरेंट शुरू हुआ: {name}।',
  'Fuselane is closing': 'Fuselane बंद हो रहा है',
  "Fuselane doesn't take this kind of file": 'Fuselane इस तरह की फ़ाइल नहीं लेता',
  '1 download added.': '1 डाउनलोड जोड़ा गया।',
  '{n} downloads added.': '{n} डाउनलोड जोड़े गए।',
  '1 download added from the Metalink.': 'Metalink से 1 डाउनलोड जोड़ा गया।',
  '{n} downloads added from the Metalink.': 'Metalink से {n} डाउनलोड जोड़े गए।',
  "it's over 4 MB": 'यह 4 MB से बड़ी है',
  "it isn't text": 'यह टेक्स्ट नहीं है',
  'nothing in it could be added': 'इसमें से कुछ भी नहीं जोड़ा जा सका',
  'not a valid torrent file': 'यह सही टोरेंट फ़ाइल नहीं है',
  "That isn't a valid .torrent file.": 'यह सही .torrent फ़ाइल नहीं है।',

  // ==== Torrents ====
  'That isn\'t a magnet link. It should start with "magnet:?xt=urn:btih:".':
    'यह मैग्नेट लिंक नहीं है। इसे "magnet:?xt=urn:btih:" से शुरू होना चाहिए।',
  'That torrent file is {n} bytes; the limit is 8 MiB, so it is probably not a torrent file.':
    'यह टोरेंट फ़ाइल {n} बाइट की है, जबकि सीमा 8 MiB है, इसलिए शायद यह टोरेंट फ़ाइल नहीं है।',
  'No network is available for torrents. Turn on at least one network.':
    'टोरेंट के लिए कोई नेटवर्क उपलब्ध नहीं है। कम से कम एक नेटवर्क चालू करें।',
  '{name} already exists in the download folder. Move it or pick another folder.':
    '{name} डाउनलोड फ़ोल्डर में पहले से है। उसे कहीं और ले जाएं या कोई दूसरा फ़ोल्डर चुनें।',
  "This torrent isn't safe to save: {why}": 'इस टोरेंट को सेव करना सुरक्षित नहीं है: {why}',
  'Pick at least one file to download.': 'डाउनलोड के लिए कम से कम एक फ़ाइल चुनें।',
  'This torrent has no file number {n}.': 'इस टोरेंट में फ़ाइल नंबर {n} नहीं है।',
  'This torrent is already in your list.': 'यह टोरेंट पहले से आपकी सूची में है।',
  "Couldn't read the torrent: {e}": 'टोरेंट पढ़ा नहीं जा सका: {e}',
  "Couldn't read the torrent.": 'टोरेंट पढ़ा नहीं जा सका।',
  'The torrent engine failed: {e}': 'टोरेंट इंजन रुक गया: {e}',
  'Copy the whole magnet link and paste it again.':
    'पूरा मैग्नेट लिंक कॉपी करके फिर से पेस्ट करें।',
  'Pick the .torrent file, not the download itself.':
    'डाउनलोड हुई फ़ाइल नहीं, .torrent फ़ाइल चुनें।',
  'Join a Wi-Fi network, plug in Ethernet, or tether a phone.':
    'किसी Wi-Fi नेटवर्क से जुड़ें, Ethernet लगाएं, या फ़ोन टेदर करें।',
  'Move the existing file, or choose another folder.':
    'मौजूदा फ़ाइल को कहीं और ले जाएं, या कोई दूसरा फ़ोल्डर चुनें।',
  'Get the torrent from somewhere you trust.': 'टोरेंट किसी भरोसेमंद जगह से लें।',
  'The file may be damaged. Download the .torrent again.':
    'फ़ाइल शायद खराब है। .torrent फिर से डाउनलोड करें।',
  'Try again. If it keeps happening, restart Fuselane.':
    'फिर से कोशिश करें। अगर ऐसा बार-बार हो, तो Fuselane फिर से शुरू करें।',
  "Removed from the list, but 1 file couldn't be moved to the Trash.":
    'सूची से हटा दिया, पर 1 फ़ाइल ट्रैश में नहीं जा सकी।',
  "Removed from the list, but {n} files couldn't be moved to the Trash.":
    'सूची से हटा दिया, पर {n} फ़ाइलें ट्रैश में नहीं जा सकीं।',
  "Delete what's left in {path} yourself.": '{path} में जो बचा है, उसे खुद मिटाएं।',
  "That torrent isn't in your list any more.": 'यह टोरेंट अब आपकी सूची में नहीं है।',
  "Couldn't create Fuselane's torrent folder: {e}": 'Fuselane का टोरेंट फ़ोल्डर नहीं बन सका: {e}',
  'Nobody sharing this torrent could be found yet.':
    'इस टोरेंट को शेयर करने वाला अभी तक कोई नहीं मिला।',
  'Try again later; torrents with few people sharing them can take a while.':
    'बाद में फिर से कोशिश करें, कम लोगों के शेयर किए टोरेंट में समय लग सकता है।',
  "Couldn't open that file: {e}": 'वह फ़ाइल खुल नहीं सकी: {e}',
  'That is a folder, not a .torrent file.': 'यह .torrent फ़ाइल नहीं, एक फ़ोल्डर है।',
  "Couldn't read that file: {e}": 'वह फ़ाइल पढ़ी नहीं जा सकी: {e}',
  "That torrent's details were cleared. Add it again.":
    'इस टोरेंट की जानकारी मिट गई है। इसे फिर से जोड़ें।',
  "Couldn't save the torrent: {e}": 'टोरेंट सेव नहीं हो सका: {e}',
  'This torrent has finished.': 'यह टोरेंट पूरा हो चुका है।',
  "That file isn't in this torrent.": 'यह फ़ाइल इस टोरेंट में नहीं है।',
  'At least one file has to be downloaded.': 'कम से कम एक फ़ाइल डाउनलोड होनी चाहिए।',
  'Pick another file first, or remove the torrent.':
    'पहले कोई दूसरी फ़ाइल चुनें, या टोरेंट हटा दें।',
  "Fuselane no longer has this torrent's file list, so it can't delete the files.":
    'Fuselane के पास अब इस टोरेंट की फ़ाइल सूची नहीं है, इसलिए वह फ़ाइलें नहीं मिटा सकता।',
  'Delete them in your file manager.': 'इन्हें अपने फ़ाइल मैनेजर में मिटाएं।',
  "Couldn't save the setting: {e}": 'सेटिंग सेव नहीं हो सकी: {e}',
  "This torrent isn't sharing.": 'यह टोरेंट शेयर नहीं हो रहा।',
  'The sharing ratio must be between 0.1 and 10.':
    'शेयर करने का रेशियो 0.1 से 10 के बीच होना चाहिए।',
  'Sharing time must be between 1 minute and 7 days (10080 minutes).':
    'शेयर करने का समय 1 मिनट से 7 दिन (10080 मिनट) के बीच होना चाहिए।',
  '1 means upload as much as you downloaded.':
    '1 का मतलब है जितना डाउनलोड किया, उतना ही अपलोड करना।',
  'Every network has reached its data allowance, so this torrent is waiting. Raise an allowance on the Networks page, or wait for it to reset.':
    'हर नेटवर्क अपनी डेटा लिमिट तक पहुंच गया है, इसलिए यह टोरेंट इंतज़ार कर रहा है। नेटवर्क पेज पर लिमिट बढ़ाएं, या उसके रीसेट होने का इंतज़ार करें।',
  "This torrent isn't downloading now, so it can't be played from here. Open the finished file instead.":
    'यह टोरेंट अभी डाउनलोड नहीं हो रहा, इसलिए इसे यहां से चलाया नहीं जा सकता। इसकी जगह पूरी हुई फ़ाइल खोलें।',
  "{name} isn't audio or video, so there's nothing to play.":
    '{name} ऑडियो या वीडियो नहीं है, इसलिए चलाने को कुछ नहीं है।',
  "Couldn't start playing: {e}": 'चलाना शुरू नहीं हो सका: {e}',
  "Couldn't show the files: {e}": 'फ़ाइलें दिखाई नहीं जा सकीं: {e}',
  'The folders "{here}" and "{other}" differ only by upper/lower case.':
    'फ़ोल्डर "{here}" और "{other}" में सिर्फ़ छोटे/बड़े अक्षरों का फ़र्क है।',
  '"{here}" appears twice, or is both a file and a folder.':
    '"{here}" दो बार है, या फ़ाइल और फ़ोल्डर दोनों है।',
  '"{here}" and "{other}" differ only by upper/lower case and would overwrite each other.':
    '"{here}" और "{other}" में सिर्फ़ छोटे/बड़े अक्षरों का फ़र्क है, इसलिए एक दूसरे को मिटा देंगे।',

  // ==== Fuse Send (links) ====
  "Couldn't create Fuselane's send folder: {e}": 'Fuselane का भेजने वाला फ़ोल्डर नहीं बन सका: {e}',
  'Only single files can be sent for now.': 'अभी एक बार में एक ही फ़ाइल भेजी जा सकती है।',
  'Zip the folder and send the zip.': 'फ़ोल्डर को zip करके zip भेजें।',
  'Preparing stopped unexpectedly. Try again.': 'तैयारी अचानक रुक गई। फिर से कोशिश करें।',
  'That share is no longer in the list.': 'यह शेयर अब सूची में नहीं है।',
  'Fuse Send links start with https://fuselane.app/s#.':
    'Fuse Send लिंक https://fuselane.app/s# से शुरू होते हैं।',
  'Fuse Send links start with {page}.': 'Fuse Send लिंक {page} से शुरू होते हैं।',
  "That isn't a whole Fuse Send link. Copy all of it and paste it again.":
    'यह पूरा Fuse Send लिंक नहीं है। पूरा लिंक कॉपी करके फिर से पेस्ट करें।',
  "You're already receiving this link.": 'आप यह लिंक पहले से प्राप्त कर रहे हैं।',
  'Looking for the sender…': 'भेजने वाले को ढूंढ रहे हैं…',
  'Checking stopped unexpectedly.': 'जांच अचानक रुक गई।',
  'That file is no longer in the list.': 'यह फ़ाइल अब सूची में नहीं है।',
  '"{name}" was moved, deleted or changed after it was shared. Share it again for a new link.':
    'शेयर करने के बाद "{name}" को कहीं और ले जाया गया, मिटाया गया या बदला गया। नए लिंक के लिए इसे फिर से शेयर करें।',
  "This file didn't check out: the link's key doesn't match, or the data was changed. Ask the sender for a new link.":
    'यह फ़ाइल जांच में सही नहीं निकली: लिंक की कुंजी मेल नहीं खाती, या डेटा बदला गया है। भेजने वाले से नया लिंक मांगें।',
  'The file\'s name isn\'t safe to save ("{name}"). Ask the sender to rename it and share it again.':
    'फ़ाइल का नाम सेव करने के लिए सुरक्षित नहीं है ("{name}")। भेजने वाले से इसका नाम बदलकर फिर से शेयर करने को कहें।',
  'This share was made by a newer Fuselane. Update Fuselane to open it.':
    'यह शेयर Fuselane के नए वर्ज़न से बना है। इसे खोलने के लिए Fuselane अपडेट करें।',
  "This isn't a Fuse Send link. It should start with {page}#v1.":
    'यह Fuse Send लिंक नहीं है। इसे {page}#v1 से शुरू होना चाहिए।',
  "This link isn't complete. Ask the sender to copy it again.":
    'यह लिंक पूरा नहीं है। भेजने वाले से इसे फिर से कॉपी करने को कहें।',
  'This link was made by a newer Fuselane. Update Fuselane to open it.':
    'यह लिंक Fuselane के नए वर्ज़न से बना है। इसे खोलने के लिए Fuselane अपडेट करें।',
  'Fuselane couldn\'t read "{name}" ({e}). Check the file is still there and you can open it.':
    'Fuselane "{name}" पढ़ नहीं सका ({e})। देखें कि फ़ाइल अब भी वहां है और आप उसे खोल सकते हैं।',
  "This link doesn't lead to a Fuse Send share. Ask the sender for the link again.":
    'यह लिंक किसी Fuse Send शेयर तक नहीं ले जाता। भेजने वाले से लिंक फिर से मांगें।',
  "Fuselane couldn't reach the sender. Ask them to open Fuselane and keep it open until the file arrives, then try the link again. On different networks, their router may block incoming connections: turning on UPnP there, or using the same Wi-Fi, fixes it.":
    'Fuselane भेजने वाले तक नहीं पहुंच सका। उनसे कहें कि Fuselane खोलें और फ़ाइल पहुंचने तक खुला रखें, फिर लिंक दोबारा आज़माएं। अलग-अलग नेटवर्क पर उनका राउटर आने वाले कनेक्शन रोक सकता है: वहां UPnP चालू करने से, या एक ही Wi-Fi इस्तेमाल करने से यह ठीक हो जाता है।',
  "Fuselane couldn't save into {dir} ({e}). Check the folder exists and you can write to it.":
    'Fuselane {dir} में सेव नहीं कर सका ({e})। देखें कि फ़ोल्डर मौजूद है और आप उसमें लिख सकते हैं।',
  "The received file didn't match what the sender shared, so it was deleted. Ask the sender for a new link.":
    'मिली फ़ाइल भेजने वाले की शेयर की गई फ़ाइल से मेल नहीं खाती, इसलिए उसे मिटा दिया गया। भेजने वाले से नया लिंक मांगें।',
  'Not enough free space in {dir}: this file needs {needed} MB and {free} MB is free. Free up space or pick another folder.':
    '{dir} में जगह कम है: इस फ़ाइल को {needed} MB चाहिए और {free} MB खाली है। जगह खाली करें या कोई दूसरा फ़ोल्डर चुनें।',

  // ==== Nearby (computers and phones on this network) ====
  "Nearby couldn't set up this computer's identity ({e}).":
    'आस-पास इस कंप्यूटर की पहचान सेट नहीं कर सका ({e})।',
  "Nearby couldn't start receiving ({e}).": 'आस-पास से फ़ाइलें लेना शुरू नहीं हो सका ({e})।',
  'Another app may be using the port. Quit LocalSend on this computer and try again.':
    'शायद कोई दूसरा ऐप यह पोर्ट इस्तेमाल कर रहा है। इस कंप्यूटर पर LocalSend बंद करें और फिर से कोशिश करें।',
  "Devices can't be found right now: {e}. Join Wi-Fi or plug in Ethernet.":
    'अभी डिवाइस नहीं ढूंढे जा सकते: {e}। Wi-Fi से जुड़ें या Ethernet लगाएं।',
  'That request has ended.': 'यह अनुरोध खत्म हो चुका है।',
  "There's no text to send. Copy something first.":
    'भेजने के लिए कोई टेक्स्ट नहीं है। पहले कुछ कॉपी करें।',
  "That's more than 64 KB of text.": 'यह 64 KB से ज़्यादा टेक्स्ट है।',
  'Save it as a file and send the file instead.': 'इसे फ़ाइल के रूप में सेव करें और फ़ाइल भेजें।',
  "That device isn't on the network anymore.": 'यह डिवाइस अब नेटवर्क पर नहीं है।',
  'Ask them to open Fuselane or LocalSend, then try again.':
    'उनसे Fuselane या LocalSend खोलने को कहें, फिर से कोशिश करें।',
  'Fuselane couldn\'t open "{name}" ({e}).': 'Fuselane "{name}" नहीं खोल सका ({e})।',
  'Check the file is still there and pick it again.':
    'देखें कि फ़ाइल अब भी वहां है, और उसे फिर से चुनें।',
  'Pick at least one file to send.': 'भेजने के लिए कम से कम एक फ़ाइल चुनें।',
  'They cancelled it.': 'उन्होंने इसे रद्द कर दिया।',
  "Couldn't send: {e}.": 'भेजा नहीं जा सका: {e}।',
  "This computer isn't on a network a phone can reach.":
    'यह कंप्यूटर ऐसे नेटवर्क पर नहीं है जिस तक फ़ोन पहुंच सके।',
  'Join the same Wi-Fi as the phone, then try again.':
    'फ़ोन वाले Wi-Fi से ही जुड़ें, फिर से कोशिश करें।',
  "Couldn't start the phone page ({e}).": 'फ़ोन पेज शुरू नहीं हो सका ({e})।',
  'Only files can be offered for now.': 'अभी सिर्फ़ फ़ाइलें ही दी जा सकती हैं।',
  'Zip the folder and offer the zip.': 'फ़ोल्डर को zip करके zip दें।',
  "There's no text to offer.": 'देने के लिए कोई टेक्स्ट नहीं है।',
  'Type some, or copy some first.': 'पहले कुछ लिखें या कॉपी करें।',
  "That's more text than can go this way (64 KB).":
    'इस तरीके से इतना टेक्स्ट नहीं जा सकता (64 KB)।',
  'Save it to a file and offer the file instead.': 'इसे फ़ाइल में सेव करें और फ़ाइल दें।',
  'The phone page is off.': 'फ़ोन पेज बंद है।',
  'Turn it on and scan the code with the phone first.':
    'पहले इसे चालू करें और फ़ोन से कोड स्कैन करें।',
  "It didn't arrive: {why}.": 'यह नहीं पहुंचा: {why}।',
  "That folder isn't there anymore.": 'यह फ़ोल्डर अब वहां नहीं है।',
  "The folder isn't there anymore.": 'फ़ोल्डर अब वहां नहीं है।',
  'Folders are kept in sync only with trusted computers.':
    'फ़ोल्डर सिर्फ़ भरोसेमंद कंप्यूटरों के साथ सिंक रखे जाते हैं।',
  'Send that computer a file and tick Trust when it asks, then try again.':
    'उस कंप्यूटर को एक फ़ाइल भेजें और पूछे जाने पर उस पर भरोसा करना चुनें, फिर से कोशिश करें।',
  'Waiting for {device} to be on the network.': '{device} के नेटवर्क पर आने का इंतज़ार है।',
  "Couldn't send: {e}. Tried again in a moment.":
    'भेजा नहीं जा सका: {e}। थोड़ी देर में फिर कोशिश होगी।',
  "the device didn't answer": 'डिवाइस ने जवाब नहीं दिया',
  'the device answered with a different identity than it announced':
    'डिवाइस ने बताई गई पहचान से अलग पहचान के साथ जवाब दिया',
  'the other person declined': 'दूसरे व्यक्ति ने मना कर दिया',
  'the device is busy receiving something else': 'डिवाइस कुछ और लेने में व्यस्त है',
  'the device refused it ({code})': 'डिवाइस ने इसे नहीं लिया ({code})',
  cancelled: 'रद्द',
  'the other person cancelled it': 'दूसरे व्यक्ति ने इसे रद्द कर दिया',
  "the device's answer couldn't be read": 'डिवाइस का जवाब पढ़ा नहीं जा सका',
  "couldn't read {name} ({e})": '{name} पढ़ी नहीं जा सकी ({e})',
  "couldn't save it ({e})": 'इसे सेव नहीं किया जा सका ({e})',
  'it was bigger than it said': 'यह बताए गए साइज़ से बड़ी थी',
  'it was bigger than announced': 'यह बताए गए साइज़ से बड़ी थी',
  'it was smaller than announced': 'यह बताए गए साइज़ से छोटी थी',
  "it didn't arrive in full": 'यह पूरी नहीं पहुंची',
  "its checksum didn't match": 'इसका चेकसम मेल नहीं खाया',
  'that folder leads outside the save folder': 'वह फ़ोल्डर सेव वाले फ़ोल्डर से बाहर ले जाता है',

  // ==== Updates, settings and the rest ====
  "Couldn't check for updates: {e}": 'अपडेट की जांच नहीं हो सकी: {e}',
  "you're offline": 'आप ऑफ़लाइन हैं',
  "The update couldn't be installed: {e}": 'अपडेट इंस्टॉल नहीं हो सका: {e}',
  "There's no update to install.": 'इंस्टॉल करने के लिए कोई अपडेट नहीं है।',
  "Couldn't download the update: {why}.": 'अपडेट डाउनलोड नहीं हो सका: {why}।',
  'every network dropped at {size}': 'हर नेटवर्क {size} पर टूट गया',
  'Try again. It continues from where it stopped, and your downloads are safe.':
    'फिर से कोशिश करें। यह वहीं से आगे बढ़ेगा जहां रुका था, और आपके डाउनलोड सुरक्षित हैं।',
  'Try again. It picks up the check from the start, and your downloads are safe.':
    'फिर से कोशिश करें। जांच शुरू से होगी, और आपके डाउनलोड सुरक्षित हैं।',
  'Update cancelled.': 'अपडेट रद्द किया गया।',
  "Couldn't open the browser: {e}": 'ब्राउज़र नहीं खुल सका: {e}',
  // Opening the download list at launch (src-tauri/src/startup.rs)
  'This download list was made by a newer Fuselane.':
    'यह डाउनलोड सूची Fuselane के नए वर्ज़न ने बनाई है।',
  'Update Fuselane to keep going. Your downloads are kept.':
    'आगे बढ़ने के लिए Fuselane अपडेट करें। आपके डाउनलोड सुरक्षित रहेंगे।',
  "Fuselane couldn't find this computer's app data folder.":
    'Fuselane को इस कंप्यूटर का ऐप डेटा फ़ोल्डर नहीं मिला।',
  'Set FUSELANE_HOME to a folder you can write to, then open Fuselane again.':
    'FUSELANE_HOME में ऐसा फ़ोल्डर डालें जिसमें आप लिख सकते हैं, फिर Fuselane दोबारा खोलें।',
  "The drive with Fuselane's folder is full.": 'Fuselane के फ़ोल्डर वाली ड्राइव भर गई है।',
  'Free some space on it, then open Fuselane again.':
    'उसमें कुछ जगह खाली करें, फिर Fuselane दोबारा खोलें।',
  "Fuselane isn't allowed to change its folder.":
    'Fuselane को अपना फ़ोल्डर बदलने की अनुमति नहीं है।',
  'Check that your account can write to the folder, then open Fuselane again.':
    'देखें कि आपका अकाउंट इस फ़ोल्डर में लिख सकता है, फिर Fuselane दोबारा खोलें।',
  'Another program is using the download list.':
    'कोई दूसरा प्रोग्राम डाउनलोड सूची इस्तेमाल कर रहा है।',
  'Quit any other copy of Fuselane or the fuselane command, then open Fuselane again.':
    'Fuselane की कोई दूसरी कॉपी या fuselane कमांड बंद करें, फिर Fuselane दोबारा खोलें।',
  "The download list is damaged and couldn't be moved aside.":
    'डाउनलोड सूची खराब है और उसे अलग नहीं हटाया जा सका।',
  'Open the folder and move jobs.db somewhere else. Fuselane starts a new list next time.':
    'फ़ोल्डर खोलें और jobs.db को कहीं और रख दें। अगली बार Fuselane नई सूची शुरू करेगा।',
  "Fuselane couldn't open your download list.": 'Fuselane आपकी डाउनलोड सूची नहीं खोल सका।',
  'Open the folder to check it, then open Fuselane again. Copy the details if you ask for help.':
    'फ़ोल्डर खोलकर जांचें, फिर Fuselane दोबारा खोलें। मदद मांगते समय जानकारी कॉपी करके भेजें।',
  'Fuselane has no folder to show.': 'Fuselane के पास दिखाने के लिए कोई फ़ोल्डर नहीं है।',
  "Couldn't open the folder: {e}": 'फ़ोल्डर नहीं खुल सका: {e}',
  'It may not exist yet. Copy the details to see where it should be.':
    'हो सकता है यह अभी बना ही न हो। यह कहां होना चाहिए, यह देखने के लिए जानकारी कॉपी करें।',
  'Go to fuselane.app/download in your browser.': 'अपने ब्राउज़र में fuselane.app/download खोलें।',
  "Couldn't copy: {e}": 'कॉपी नहीं हो सका: {e}',
  "Couldn't open the release notes: {e}": 'रिलीज़ नोट्स नहीं खुल सके: {e}',
  "Couldn't build the report: {e}": 'रिपोर्ट नहीं बन सकी: {e}',
  'Try again, or add Fuselane to your login items yourself.':
    'फिर से कोशिश करें, या Fuselane को खुद अपने लॉगिन आइटम में जोड़ें।',
  "Fuselane couldn't save that: {e}": 'Fuselane इसे सेव नहीं कर सका: {e}',
  "Fuselane couldn't save the setting: {e}": 'Fuselane सेटिंग सेव नहीं कर सका: {e}',
  'Port {port} is in use by another program (aria2 itself, perhaps). Pick another port.':
    'पोर्ट {port} कोई दूसरा प्रोग्राम (शायद खुद aria2) इस्तेमाल कर रहा है। कोई दूसरा पोर्ट चुनें।',
  "It couldn't start ({e}).": 'यह शुरू नहीं हो सका ({e})।',
  'Pick a port from 1024 to 65535.': '1024 से 65535 तक का कोई पोर्ट चुनें।',
  '6800 is what aria2 tools expect.': 'aria2 टूल 6800 की उम्मीद करते हैं।',
  "Fuselane couldn't tell where it is installed, so browsers can't find it.":
    'Fuselane पता नहीं लगा सका कि वह कहां इंस्टॉल है, इसलिए ब्राउज़र उसे नहीं ढूंढ सकते।',
  'Fuselane is running from the disk image or a temporary copy. Drag it into Applications and open it from there so browsers can find it.':
    'Fuselane डिस्क इमेज या किसी अस्थायी कॉपी से चल रहा है। इसे Applications में खींचें और वहीं से खोलें, ताकि ब्राउज़र इसे ढूंढ सकें।',
  // ==== Flatpak ====
  'Updates come through your software centre (Flatpak).':
    'अपडेट आपके सॉफ़्टवेयर सेंटर (Flatpak) से आते हैं।',
  'Update Fuselane there, or run flatpak update.':
    'Fuselane को वहीं अपडेट करें, या flatpak update चलाएं।',
  "The Flatpak version can't put the computer to sleep or shut it down.":
    'Flatpak वर्ज़न कंप्यूटर को स्लीप या बंद नहीं कर सकता।',
  'Pick Quit or Nothing, or use the .deb or AppImage for this.':
    'बंद करें या कुछ नहीं चुनें, या इसके लिए .deb या AppImage इस्तेमाल करें।',
  "Your system didn't accept the change ({e}).": 'आपके सिस्टम ने यह बदलाव नहीं माना ({e})।',
  'Allow Fuselane to run in the background in your system settings, then try again.':
    'सिस्टम सेटिंग्स में Fuselane को बैकग्राउंड में चलने दें, फिर से कोशिश करें।',
  'Start Fuselane when you sign in, so scheduled downloads run.':
    'साइन इन करने पर Fuselane शुरू करें, ताकि तय किए गए डाउनलोड चल सकें।',
}
